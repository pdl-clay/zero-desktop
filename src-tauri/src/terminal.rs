use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use portable_pty::{native_pty_system, Child, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex as AsyncMutex;

/// Result of spawning a terminal, returned to the frontend so the tab strip
/// can show something more useful than the raw key (pid for diagnostics,
/// shell for the tab label).
#[derive(Debug, Clone, serde::Serialize)]
pub struct TerminalSpawnInfo {
    pub key: String,
    pub pid: Option<u32>,
    pub shell: String,
}

/// Snapshot of one live terminal, returned by `list_terminals` for frontend
/// reconciliation (mirrors `LiveSessionInfo` in bridge.rs).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LiveTerminalInfo {
    pub key: String,
    pub cwd: String,
    pub live: bool,
}

/// Best-effort default shell, purely for the tab label - the actual process
/// is always launched via `CommandBuilder::new_default_prog()`, which does
/// its own (more thorough) $SHELL/passwd-database resolution on the Rust
/// side of portable-pty; this is not required to match that exactly.
fn shell_display_name() -> String {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
    shell.rsplit('/').next().unwrap_or(&shell).to_string()
}

/// Consumes `chunk`, prepending any leftover bytes from a previous read that
/// were a truncated multi-byte UTF-8 sequence. PTY output is arbitrary bytes
/// (ANSI escapes, no line discipline) read in fixed-size chunks, so a
/// multi-byte character can legitimately be split across two `read()` calls;
/// without this, `String::from_utf8_lossy` would replace the truncated tail
/// with U+FFFD every single time instead of completing it on the next chunk.
/// Only genuinely truncated sequences (`error_len() == None`, and at most 3
/// leftover bytes - the max a UTF-8 sequence can be missing) are buffered;
/// anything else is real invalid data and is lossy-replaced immediately so a
/// bad byte can never wedge the terminal by never being flushed.
fn drain_utf8(leftover: &mut Vec<u8>, chunk: &[u8]) -> String {
    let mut buf = std::mem::take(leftover);
    buf.extend_from_slice(chunk);
    match std::str::from_utf8(&buf) {
        Ok(s) => s.to_string(),
        Err(e) => {
            let valid_up_to = e.valid_up_to();
            let (valid, rest) = buf.split_at(valid_up_to);
            let mut s = String::from_utf8_lossy(valid).into_owned();
            if e.error_len().is_none() && rest.len() <= 3 {
                *leftover = rest.to_vec();
            } else if !rest.is_empty() {
                s.push_str(&String::from_utf8_lossy(rest));
            }
            s
        }
    }
}

/// One live PTY-backed terminal. `master` is kept alive for the lifetime of
/// the terminal purely because dropping it would tear down the pty; the
/// reader/writer/killer are the only things actually used day-to-day.
struct TerminalHandle {
    #[allow(dead_code)]
    master: Box<dyn MasterPty + Send>,
    writer: StdMutex<Box<dyn Write + Send>>,
    killer: StdMutex<Box<dyn ChildKiller + Send + Sync>>,
    cwd: String,
}

/// Manages one PTY-backed shell process per open terminal tab, keyed by a
/// frontend-owned uuid - same shape as `ZeroBridge`'s session map, but for
/// real shells instead of `zero acp` processes. Unlike `ZeroBridge`, there is
/// no respawn-on-demand: closing a tab kills its shell for good, exactly like
/// closing a real terminal window.
pub struct TerminalManager {
    app: AppHandle,
    terminals: Arc<AsyncMutex<HashMap<String, TerminalHandle>>>,
}

impl TerminalManager {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            terminals: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    pub async fn spawn(
        &self,
        key: String,
        cwd: String,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalSpawnInfo, String> {
        let mut terminals = self.terminals.lock().await;
        if terminals.contains_key(&key) {
            return Err(format!("Terminal {key} already exists"));
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open pty: {e}"))?;

        let mut cmd = CommandBuilder::new_default_prog();
        cmd.cwd(&cwd);
        // Inherited from get_base_env() by default; GUI apps are usually
        // launched with no TERM at all (no controlling terminal), which
        // leaves shells assuming a dumb terminal (no color, broken cursor
        // movement) - xterm.js expects proper terminfo support.
        cmd.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn shell: {e}"))?;
        // Drop our handle to the slave side right away: on Unix, the
        // master's reader only sees EOF once every slave fd is closed. As
        // long as we (the parent) keep a slave fd open too, the master
        // never sees EOF after the child exits and the reader thread would
        // block forever instead of noticing the shell died.
        drop(pair.slave);

        let pid = child.process_id();
        let killer = child.clone_killer();

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone pty reader: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take pty writer: {e}"))?;

        let handle = TerminalHandle {
            master: pair.master,
            writer: StdMutex::new(writer),
            killer: StdMutex::new(killer),
            cwd: cwd.clone(),
        };
        terminals.insert(key.clone(), handle);
        drop(terminals);

        self.spawn_reader_thread(key.clone(), reader, child);

        Ok(TerminalSpawnInfo {
            key,
            pid,
            shell: shell_display_name(),
        })
    }

    /// The only thread reading a given pty's output. Runs on a plain OS
    /// thread (not `tokio::spawn`) because `portable-pty`'s `Read`/`Child`
    /// are blocking, synchronous APIs with no async equivalent. Reaps the
    /// child (`child.wait()`) once the pty reports EOF, removes the
    /// terminal from the map, and emits `terminal:exit` - this is the one
    /// and only place entries are removed, whether the shell exited on its
    /// own (`exit`, crash) or was killed via `kill_terminal`/`kill_all`.
    fn spawn_reader_thread(
        &self,
        key: String,
        mut reader: Box<dyn Read + Send>,
        mut child: Box<dyn Child + Send + Sync>,
    ) {
        let app = self.app.clone();
        let terminals = self.terminals.clone();

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            let mut leftover: Vec<u8> = Vec::new();

            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let text = drain_utf8(&mut leftover, &buf[..n]);
                        if !text.is_empty() {
                            let _ = app.emit(
                                "terminal:data",
                                serde_json::json!({ "key": key, "data": text }),
                            );
                        }
                    }
                    Err(_) => break,
                }
            }

            let exit_code = child.wait().ok().map(|status| status.exit_code());
            terminals.blocking_lock().remove(&key);
            let _ = app.emit(
                "terminal:exit",
                serde_json::json!({ "key": key, "exitCode": exit_code }),
            );
        });
    }

    pub async fn write(&self, key: String, data: String) -> Result<(), String> {
        let terminals = self.terminals.clone();
        tokio::task::spawn_blocking(move || {
            let terminals = terminals.blocking_lock();
            let handle = terminals
                .get(&key)
                .ok_or_else(|| format!("No terminal for key {key}"))?;
            let mut writer = handle
                .writer
                .lock()
                .map_err(|_| "terminal writer lock poisoned".to_string())?;
            writer
                .write_all(data.as_bytes())
                .map_err(|e| format!("Failed to write to terminal: {e}"))?;
            writer
                .flush()
                .map_err(|e| format!("Failed to flush terminal: {e}"))
        })
        .await
        .map_err(|e| format!("Write task panicked: {e}"))?
    }

    pub async fn resize(&self, key: String, cols: u16, rows: u16) -> Result<(), String> {
        let terminals = self.terminals.clone();
        tokio::task::spawn_blocking(move || {
            let terminals = terminals.blocking_lock();
            let handle = terminals
                .get(&key)
                .ok_or_else(|| format!("No terminal for key {key}"))?;
            handle
                .master
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Failed to resize terminal: {e}"))
        })
        .await
        .map_err(|e| format!("Resize task panicked: {e}"))?
    }

    /// Sends the kill signal and returns - it does not wait for the process
    /// to actually die (that happens in `spawn_reader_thread`, which reaps it
    /// and removes the map entry once its read loop sees EOF).
    pub async fn kill(&self, key: String) -> Result<(), String> {
        let terminals = self.terminals.clone();
        tokio::task::spawn_blocking(move || {
            let terminals = terminals.blocking_lock();
            if let Some(handle) = terminals.get(&key) {
                let mut killer = handle
                    .killer
                    .lock()
                    .map_err(|_| "terminal killer lock poisoned".to_string())?;
                killer
                    .kill()
                    .map_err(|e| format!("Failed to kill terminal: {e}"))?;
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| format!("Kill task panicked: {e}"))?
    }

    pub async fn list(&self) -> Vec<LiveTerminalInfo> {
        self.terminals
            .lock()
            .await
            .iter()
            .map(|(key, h)| LiveTerminalInfo {
                key: key.clone(),
                cwd: h.cwd.clone(),
                live: true,
            })
            .collect()
    }

    /// Kills every live terminal without waiting for them to exit - used on
    /// app quit so no orphan shells remain. Mirrors `ZeroBridge::kill_all`,
    /// but doesn't drain/await reaping: the process is exiting anyway, and
    /// the detached reader threads (which own each `Child`) would have no
    /// chance to run their cleanup after `RunEvent::Exit` returns regardless.
    pub async fn kill_all(&self) {
        let terminals = self.terminals.lock().await;
        for handle in terminals.values() {
            if let Ok(mut killer) = handle.killer.lock() {
                let _ = killer.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drain_utf8_passes_through_ascii() {
        let mut leftover = Vec::new();
        assert_eq!(drain_utf8(&mut leftover, b"hello"), "hello");
        assert!(leftover.is_empty());
    }

    #[test]
    fn drain_utf8_buffers_truncated_multibyte_sequence() {
        // "é" is 0xC3 0xA9 in UTF-8; keep only the first (leading) byte of
        // that sequence in this chunk so it's genuinely truncated.
        let full = "café".as_bytes().to_vec();
        let (first, _rest) = full.split_at(full.len() - 1);

        let mut leftover = Vec::new();
        let out1 = drain_utf8(&mut leftover, first);
        assert_eq!(out1, "caf");
        assert_eq!(leftover, vec![0xC3]);
    }

    #[test]
    fn drain_utf8_completes_sequence_on_next_chunk() {
        let full = "café".as_bytes().to_vec();
        let split_at = full.len() - 1;
        let (first, second) = full.split_at(split_at);

        let mut leftover = Vec::new();
        let out1 = drain_utf8(&mut leftover, first);
        let out2 = drain_utf8(&mut leftover, second);
        assert_eq!(format!("{out1}{out2}"), "café");
        assert!(leftover.is_empty());
    }

    #[test]
    fn drain_utf8_replaces_genuinely_invalid_bytes() {
        let mut leftover = Vec::new();
        let out = drain_utf8(&mut leftover, &[0xFF, 0xFE, b'x']);
        assert!(out.ends_with('x'));
        assert!(leftover.is_empty());
    }
}
