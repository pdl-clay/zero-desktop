/**
 * Detect tool names that edit or write files. Handles both zero's native
 * `edit_file` / `write_file` and MCP-backed variants such as
 * `mcp_filesystem_edit_file`.
 */
export function isEditTool(toolName) {
  return (
    toolName === "edit_file" ||
    toolName === "write_file" ||
    toolName.endsWith("_edit_file") ||
    toolName.endsWith("_write_file")
  );
}

/**
 * Extract the old/new string pair from an edit_file tool call. The CLI uses
 * `old_str`/`new_str` in some contexts and `old_string`/`new_string` in others,
 * so we accept both.
 */
export function getEditStrings(input) {
  if (!input || typeof input !== "object") return null;
  const oldStr =
    typeof input.old_str === "string"
      ? input.old_str
      : typeof input.old_string === "string"
        ? input.old_string
        : null;
  const newStr =
    typeof input.new_str === "string"
      ? input.new_str
      : typeof input.new_string === "string"
        ? input.new_string
        : null;
  if (oldStr === null || newStr === null) return null;
  return { oldStr, newStr };
}
