// Pure byte readers for the formats the site ships: a PNG's IHDR and pixels, and an ICO's
// directory. Shared by the drift check (scripts/brand.mjs) and the e2e asset test, so both
// read the files the same way.
import { inflateSync } from 'node:zlib';

const SIGNATURE = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/** The PNG's chunks, in order, after a checked signature. */
function chunks(bytes) {
  if (bytes.length < 8 || SIGNATURE.some((b, i) => bytes[i] !== b)) throw new Error('not a PNG (bad signature)');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out = [];
  for (let at = 8; at + 12 <= bytes.length; ) {
    const length = view.getUint32(at);
    const type = String.fromCharCode(...bytes.subarray(at + 4, at + 8));
    out.push({ type, data: bytes.subarray(at + 8, at + 8 + length) });
    at += 12 + length;
  }
  return out;
}

/** { width, height, bitDepth, colorType, interlace } from the first chunk, which must be IHDR. */
export function ihdr(bytes) {
  const first = chunks(bytes)[0];
  if (!first || first.type !== 'IHDR' || first.data.length !== 13) throw new Error('PNG has no IHDR first');
  const v = new DataView(first.data.buffer, first.data.byteOffset, 13);
  return { width: v.getUint32(0), height: v.getUint32(4), bitDepth: first.data[8], colorType: first.data[9], interlace: first.data[12] };
}

const CHANNELS = { 0: 1, 2: 3, 4: 2, 6: 4 };

/** RGBA-or-fewer 8-bit, non-interlaced pixels (what resvg writes), unfiltered. */
export function pixels(bytes) {
  const h = ihdr(bytes);
  const channels = CHANNELS[h.colorType];
  if (h.bitDepth !== 8 || !channels || h.interlace !== 0) throw new Error(`unsupported PNG layout ${JSON.stringify(h)}`);
  const idat = Buffer.concat(chunks(bytes).filter((c) => c.type === 'IDAT').map((c) => c.data));
  const raw = inflateSync(idat);
  const stride = h.width * channels;
  const out = new Uint8Array(stride * h.height);
  for (let y = 0; y < h.height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? out[y * stride + x - channels] : 0;
      const b = y > 0 ? out[(y - 1) * stride + x] : 0;
      const c = x >= channels && y > 0 ? out[(y - 1) * stride + x - channels] : 0;
      out[y * stride + x] = (line[x] + predict(filter, a, b, c)) & 0xff;
    }
  }
  return { ...h, channels, data: out };
}

function predict(filter, a, b, c) {
  switch (filter) {
    case 0: return 0;
    case 1: return a;
    case 2: return b;
    case 3: return (a + b) >> 1;
    case 4: {
      const p = a + b - c;
      const [pa, pb, pc] = [Math.abs(p - a), Math.abs(p - b), Math.abs(p - c)];
      return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
    }
    default: throw new Error(`bad PNG filter ${filter}`);
  }
}

/**
 * How two PNGs differ: identical bytes, or the share of channel values that differ by more than
 * `tolerance` (anti-aliasing can round differently across CPU architectures; an edit cannot hide
 * under that). Different dimensions or layouts are never close.
 */
export function compare(expected, actual, tolerance) {
  if (Buffer.compare(Buffer.from(expected), Buffer.from(actual)) === 0) return { exact: true, close: true, share: 0, max: 0 };
  const [e, a] = [pixels(expected), pixels(actual)];
  if (e.width !== a.width || e.height !== a.height || e.channels !== a.channels) return { exact: false, close: false, share: 1, max: 255 };
  let [over, max] = [0, 0];
  for (let i = 0; i < e.data.length; i++) {
    const d = Math.abs(e.data[i] - a.data[i]);
    if (d > max) max = d;
    if (d > tolerance) over++;
  }
  return { exact: false, close: over === 0, share: over / e.data.length, max };
}

/** An ICO's directory: one { width, height, bpp, png } per image (0 in the header means 256). */
export function icoEntries(bytes) {
  const v = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.length < 6 || v.getUint16(0, true) !== 0 || v.getUint16(2, true) !== 1) throw new Error('not an ICO (bad header)');
  const out = [];
  for (let i = 0; i < v.getUint16(4, true); i++) {
    const at = 6 + i * 16;
    const size = v.getUint32(at + 8, true);
    const offset = v.getUint32(at + 12, true);
    out.push({ width: bytes[at] || 256, height: bytes[at + 1] || 256, bpp: v.getUint16(at + 6, true), image: bytes.subarray(offset, offset + size) });
  }
  return out;
}

/** An ICO holding each PNG as is (the PNG-in-ICO form every current browser and OS reads). */
export function icoPack(pngs) {
  const header = Buffer.alloc(6 + pngs.length * 16);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(pngs.length, 4);
  let offset = header.length;
  pngs.forEach((png, i) => {
    const { width, height } = ihdr(png);
    const at = 6 + i * 16;
    header.writeUInt8(width >= 256 ? 0 : width, at);
    header.writeUInt8(height >= 256 ? 0 : height, at + 1);
    header.writeUInt16LE(1, at + 4);
    header.writeUInt16LE(32, at + 6);
    header.writeUInt32LE(png.length, at + 8);
    header.writeUInt32LE(offset, at + 12);
    offset += png.length;
  });
  return Buffer.concat([header, ...pngs.map((p) => Buffer.from(p))]);
}
