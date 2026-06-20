// Generates a 1024x1024 source PNG for the app icon with no external deps.
// A diagonal brand gradient with a bold "D" punched out via a simple mask.
// Run `npm run tauri icon scripts/app-icon.png` afterwards to produce all sizes.
import { deflateSync } from "node:zlib";
import { writeFileSync } from "node:fs";

const S = 1024;
const buf = Buffer.alloc(S * S * 4);

// Brand colors (Fluent-ish blurple → berry).
const c1 = [79, 70, 229];
const c2 = [192, 38, 211];

// A crude "D" glyph mask: a vertical bar plus a half-ellipse bowl.
function inD(x, y) {
  const nx = x / S;
  const ny = y / S;
  // padding
  if (nx < 0.28 || nx > 0.78 || ny < 0.22 || ny > 0.78) return false;
  const stem = nx >= 0.28 && nx <= 0.4; // left vertical stem
  // bowl: outer ellipse minus inner ellipse, only right half
  const cx = 0.4,
    cy = 0.5;
  const ox = 0.4,
    oy = 0.3;
  const ix = 0.27,
    iy = 0.18;
  const outer = ((nx - cx) / ox) ** 2 + ((ny - cy) / oy) ** 2 <= 1;
  const inner = ((nx - cx) / ix) ** 2 + ((ny - cy) / iy) ** 2 <= 1;
  const bowl = outer && !inner && nx >= 0.4;
  return stem || bowl;
}

for (let y = 0; y < S; y++) {
  for (let x = 0; x < S; x++) {
    const t = (x + y) / (2 * S);
    const i = (y * S + x) * 4;
    const onGlyph = inD(x, y);
    if (onGlyph) {
      buf[i] = 255;
      buf[i + 1] = 255;
      buf[i + 2] = 255;
      buf[i + 3] = 255;
    } else {
      buf[i] = Math.round(c1[0] + (c2[0] - c1[0]) * t);
      buf[i + 1] = Math.round(c1[1] + (c2[1] - c1[1]) * t);
      buf[i + 2] = Math.round(c1[2] + (c2[2] - c1[2]) * t);
      buf[i + 3] = 255;
    }
  }
}

// Assemble a minimal PNG (IHDR, IDAT, IEND) with one zlib stream.
function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length, 0);
  const typeBuf = Buffer.from(type, "ascii");
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuf, data])) >>> 0, 0);
  return Buffer.concat([len, typeBuf, data, crc]);
}

function crc32(b) {
  let c = ~0;
  for (let i = 0; i < b.length; i++) {
    c ^= b[i];
    for (let k = 0; k < 8; k++) c = c & 1 ? (c >>> 1) ^ 0xedb88320 : c >>> 1;
  }
  return ~c;
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8; // bit depth
ihdr[9] = 6; // RGBA
// add a filter byte (0) per scanline
const raw = Buffer.alloc(S * (S * 4 + 1));
for (let y = 0; y < S; y++) {
  raw[y * (S * 4 + 1)] = 0;
  buf.copy(raw, y * (S * 4 + 1) + 1, y * S * 4, (y + 1) * S * 4);
}
const png = Buffer.concat([
  Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
  chunk("IHDR", ihdr),
  chunk("IDAT", deflateSync(raw)),
  chunk("IEND", Buffer.alloc(0)),
]);

const out = new URL("./app-icon.png", import.meta.url);
writeFileSync(out, png);
console.log("wrote", out.pathname, png.length, "bytes");
