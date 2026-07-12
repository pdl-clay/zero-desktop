/**
 * Check whether a MIME type represents an image that can be previewed inline.
 * @param {string} mimeType
 * @returns {boolean}
 */
export function isImageMimeType(mimeType) {
  return typeof mimeType === "string" && mimeType.startsWith("image/");
}

/**
 * Check whether a MIME type represents a readable text file.
 * @param {string} mimeType
 * @returns {boolean}
 */
export function isTextMimeType(mimeType) {
  if (typeof mimeType !== "string") return false;
  if (mimeType.startsWith("text/")) return true;
  return ["application/json", "application/yaml", "application/xml"].includes(mimeType);
}

/**
 * Pick a Material icon name for a file based on its MIME type or extension.
 * @param {string} mimeType
 * @param {string} name
 * @returns {string}
 */
export function getFileIcon(mimeType, name = "") {
  if (isImageMimeType(mimeType)) return "image";

  const ext = (name.split(".").pop() || "").toLowerCase();
  const langIcons = {
    py: "code",
    go: "code",
    rs: "code",
    java: "code",
    kt: "code",
    swift: "code",
    c: "code",
    cpp: "code",
    cc: "code",
    cxx: "code",
    h: "code",
    hpp: "code",
    rb: "code",
    php: "code",
    sh: "terminal",
    sql: "storage",
    js: "javascript",
    ts: "javascript",
    jsx: "javascript",
    tsx: "javascript",
    css: "style",
    html: "html",
    htm: "html",
    md: "description",
    json: "data_object",
    yaml: "description",
    yml: "description",
    xml: "description",
    csv: "table_chart",
    txt: "description",
    dockerfile: "view_in_ar",
  };

  return langIcons[ext] || "insert_drive_file";
}
