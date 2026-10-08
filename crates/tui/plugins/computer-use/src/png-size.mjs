// Minimal PNG dimension reader: parses the IHDR chunk only. Lives in src/
// (not scripts/lib) because src/ ships standalone to remote agents.
import fs from "node:fs";

const SIGNATURE = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);

/** @returns {{w:number, h:number} | null} pixel dimensions, or null when the file is not a PNG. */
export function pngSize(file) {
  let fd;
  try {
    fd = fs.openSync(file, "r");
    const head = Buffer.alloc(24);
    if (fs.readSync(fd, head, 0, 24, 0) < 24) return null;
    if (!head.subarray(0, 8).equals(SIGNATURE)) return null;
    if (head.toString("latin1", 12, 16) !== "IHDR") return null;
    return { w: head.readUInt32BE(16), h: head.readUInt32BE(20) };
  } catch {
    return null;
  } finally {
    try { if (fd != null) fs.closeSync(fd); } catch {}
  }
}
