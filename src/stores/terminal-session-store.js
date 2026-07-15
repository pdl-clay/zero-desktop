import { defineStore } from "pinia";
import { markRaw } from "vue";
import {
  spawnTerminal,
  writeTerminal,
  resizeTerminal,
  killTerminal,
  onTerminalData,
  onTerminalExit,
} from "@/services/terminal";
import { useTerminalRuntimeStore } from "@/stores/terminal-runtime-store";

// One dynamic store instance per open terminal tab, keyed the same way
// zero-session-store.js is - owns the live xterm.js Terminal/FitAddon
// instances (created by TerminalHost.vue, which needs $q for theming) and
// the backend PTY process lifecycle for this one tab.
export function useTerminalSessionStore(key) {
  return defineStore(`terminal-session:${key}`, {
    state: () => ({
      terminalKey: key,
      cwd: null,
      pid: null,
      shell: null,
      status: "idle", // idle | spawning | running | exited
      exitCode: null,
      // Live objects, not plain data - kept out of Vue's reactivity via
      // markRaw() in attach() below. xterm.js Terminal instances have
      // internal circular references and run their own render loop; letting
      // Vue proxy them is wasted work at best and risks breaking identity
      // checks xterm does internally at worst.
      term: null,
      fitAddon: null,
      _unlistenData: null,
      _unlistenExit: null,
    }),

    actions: {
      // TerminalHost.vue creates the xterm.js instance (it needs $q for
      // theming, which isn't available from inside a Pinia action) and
      // hands it here right after `term.open(el)`, before spawn() below
      // actually starts the backend shell.
      attach(term, fitAddon) {
        if (this.term) return;
        this.term = markRaw(term);
        this.fitAddon = markRaw(fitAddon);
      },

      async spawn(cwd) {
        if (this.status !== "idle" || !this.term) return;
        this.cwd = cwd;
        this.status = "spawning";
        const runtime = useTerminalRuntimeStore();

        this._unlistenData = await onTerminalData((event) => {
          if (event.payload.key === this.terminalKey) {
            this.term.write(event.payload.data);
          }
        });
        this._unlistenExit = await onTerminalExit((event) => {
          if (event.payload.key !== this.terminalKey) return;
          this.status = "exited";
          this.exitCode = event.payload.exitCode;
          runtime.registerMeta(this.terminalKey, { status: "exited" });
        });

        const info = await spawnTerminal(this.terminalKey, cwd, this.term.cols, this.term.rows);
        this.pid = info.pid;
        this.shell = info.shell;
        this.status = "running";
        runtime.registerMeta(this.terminalKey, {
          cwd,
          pid: info.pid,
          shell: info.shell,
          title: info.shell,
          status: "running",
        });

        this.term.onData((data) => {
          writeTerminal(this.terminalKey, data);
        });
      },

      resize(cols, rows) {
        if (this.status !== "running") return;
        return resizeTerminal(this.terminalKey, cols, rows);
      },

      async kill() {
        this._unlistenData?.();
        this._unlistenExit?.();
        this._unlistenData = null;
        this._unlistenExit = null;
        if (this.status === "running" || this.status === "spawning") {
          try {
            await killTerminal(this.terminalKey);
          } catch {
            // Already gone - that's the desired end state anyway.
          }
        }
        this.term?.dispose();
      },

      // Text to insert into a chat panel's draft when citing this
      // terminal's output: the active selection if there is one, else the
      // currently visible viewport. Deliberately plain text (no ANSI/color
      // reconstruction, unlike @xterm/addon-serialize) since it's headed
      // into a fenced code block, not back into another terminal.
      extractCiteText() {
        if (!this.term) return "";
        const selection = this.term.getSelection();
        if (selection && selection.trim().length > 0) {
          return selection;
        }
        const buffer = this.term.buffer.active;
        const lines = [];
        for (let i = 0; i < this.term.rows; i++) {
          const line = buffer.getLine(buffer.viewportY + i);
          if (line) lines.push(line.translateToString(true));
        }
        // Trailing blank rows are just unused viewport below the shell's
        // actual output, not something the user asked to cite.
        while (lines.length > 0 && lines[lines.length - 1] === "") {
          lines.pop();
        }
        return lines.join("\n");
      },
    },
  })();
}
