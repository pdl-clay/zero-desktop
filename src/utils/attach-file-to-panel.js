import { readFileAttachment } from "@/services/zero";
import { useZeroSessionStore } from "@/stores/zero-session-store";

/**
 * Reads a file from disk and sets it as a chat panel's pending attachment -
 * shared by the file explorer's click-to-cite (targets the focused panel)
 * and drag-and-drop-to-cite (targets whichever panel the file was dropped
 * on) actions, so neither has to duplicate the read+set logic.
 * @param {string} path - absolute path to the file
 * @param {string} sessionKey - the target chat panel's session key
 * @returns {Promise<void>}
 */
export async function attachFileToPanel(path, sessionKey) {
  if (!path || !sessionKey) return;
  const attachment = await readFileAttachment(path);
  useZeroSessionStore(sessionKey).pendingAttachment = attachment;
}
