/**
 * Convert a base64 string (no `data:` prefix) into a `blob:` object URL.
 * Preferred over a `data:` URI for previewing locally-read image bytes -
 * keeps multi-MB base64 strings out of Vue's reactive state/DOM attributes
 * and avoids relying on `data:` URI support in `img-src` CSP matching.
 * Caller owns the returned URL and must revoke it (`URL.revokeObjectURL`)
 * once it is no longer displayed, or it leaks for the page's lifetime.
 * @param {string} base64
 * @param {string} mimeType
 * @returns {string} object URL
 */
export function base64ToObjectUrl(base64, mimeType) {
  const bytes = base64ToUint8Array(base64);
  const blob = new Blob([bytes], { type: mimeType });
  return URL.createObjectURL(blob);
}

/**
 * Decode a standard base64 string (no `data:` prefix) into a Uint8Array.
 * Uses the browser's binary decoder, which is safer than atob+charCodeAt
 * for arbitrary binary image data.
 * @param {string} base64
 * @returns {Uint8Array}
 */
export function base64ToUint8Array(base64) {
  const binaryString = atob(base64);
  const len = binaryString.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binaryString.charCodeAt(i);
  }
  return bytes;
}

/**
 * Build a `data:` URI from raw base64 image data.
 * Useful as a fallback when blob URLs fail or for tiny thumbnails.
 * @param {string} base64
 * @param {string} mimeType
 * @returns {string}
 */
export function base64ToDataUri(base64, mimeType) {
  return `data:${mimeType};base64,${base64}`;
}
