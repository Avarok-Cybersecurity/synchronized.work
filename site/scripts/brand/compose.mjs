// Pure: every derived identity file as { file, kind, ... } from the master logo and brand.json.
// No I/O here; scripts/brand.mjs reads the inputs, renders and writes or checks.

/** The master's inner markup and its square side, from `<svg viewBox="0 0 S S">…</svg>`. */
export function mark(logoSvg) {
  const open = logoSvg.match(/^<svg\b[^>]*\bviewBox="0 0 (\d+(?:\.\d+)?) (\d+(?:\.\d+)?)"[^>]*>/);
  if (!open || open[1] !== open[2]) throw new Error('logo.svg must open with a square viewBox "0 0 S S"');
  const close = logoSvg.trimEnd();
  if (!close.endsWith('</svg>')) throw new Error('logo.svg must end with </svg>');
  const inner = close.slice(open[0].length, -'</svg>'.length);
  // The tile is the master's leading <rect>s; the glyph is everything after them.
  const glyph = inner.replace(/^(?:<rect\b[^>]*\/>)+/, '');
  if (glyph === inner) throw new Error('logo.svg must start with its tile <rect>s');
  return { side: Number(open[1]), inner, glyph };
}

/** The mark scaled to `size` px with its top-left at (x, y). */
function placed(m, x, y, size, markup = m.inner) {
  return `<g transform="translate(${x} ${y}) scale(${size / m.side})">${markup}</g>`;
}

/** The glyph alone on a full-bleed square of `fill`, as if the tile covered `fraction` of it. */
function fullBleed(m, side, fill, fraction) {
  const size = side * fraction;
  const at = (side - size) / 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${side}" height="${side}" viewBox="0 0 ${side} ${side}"><rect width="${side}" height="${side}" fill="${fill}"/>${placed(m, at, at, size, m.glyph)}</svg>`;
}

/** The mark alone at `side` px, transparent outside its rounded tile. */
function plain(m, side) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${side}" height="${side}" viewBox="0 0 ${side} ${side}">${placed(m, 0, 0, side)}</svg>`;
}

const escapeXml = (s) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/'/g, '&apos;');

/** 1200×630: the mark, the name and the tagline on the dark page colour. */
function ogImage(m, brand) {
  const c = brand.colors;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630">
<rect width="1200" height="630" fill="${c.pageDark}"/>
<rect x="0" y="606" width="1200" height="24" fill="${c.primary}"/>
<rect x="0" y="606" width="420" height="24" fill="${c.green}"/>
${placed(m, 96, 199, 208)}
<text x="352" y="296" font-family="Inter" font-weight="700" font-size="78" fill="${c.inkDark}">${escapeXml(brand.name)}</text>
<text x="354" y="376" font-family="Inter" font-weight="400" font-size="40" fill="${c.bodyDark}">${escapeXml(brand.tagline)}</text>
</svg>`;
}

/** The web app manifest (the site's PWA install metadata). */
export function manifest(brand) {
  const icon = (src, sizes, purpose) => ({ src, sizes, type: 'image/png', purpose });
  return {
    id: '/',
    name: brand.name,
    short_name: brand.shortName,
    description: brand.description,
    start_url: '/',
    scope: '/',
    display: 'standalone',
    theme_color: brand.colors.pageLight,
    background_color: brand.colors.pageLight,
    icons: [
      icon('/icon-192.png', '192x192', 'any'),
      icon('/icon-512.png', '512x512', 'any'),
      icon('/icon-maskable-512.png', '512x512', 'maskable'),
    ],
  };
}

/** The maskable safe zone is the centred circle of radius 40%: the mark keeps to 80%. */
const MASKABLE_FRACTION = 0.8;
/** iOS rounds the tile itself; the mark is drawn a little inset so its corners are not cut. */
const APPLE_FRACTION = 0.86;
export const ICO_SIZES = [16, 32, 48];

/** Every derived file. `png` entries are rendered from `svg`; `ico` packs one PNG per size. */
export function outputs(logoSvg, brand) {
  const m = mark(logoSvg);
  const png = (file, svg) => ({ file, kind: 'png', svg });
  return [
    { file: 'favicon.svg', kind: 'text', text: logoSvg },
    { file: 'favicon.ico', kind: 'ico', svgs: ICO_SIZES.map((s) => plain(m, s)) },
    png('apple-touch-icon.png', fullBleed(m, 180, brand.colors.primary, APPLE_FRACTION)),
    png('icon-192.png', plain(m, 192)),
    png('icon-512.png', plain(m, 512)),
    png('icon-maskable-512.png', fullBleed(m, 512, brand.colors.primary, MASKABLE_FRACTION)),
    png('og-image.png', ogImage(m, brand)),
    { file: 'site.webmanifest', kind: 'text', text: `${JSON.stringify(manifest(brand), null, 2)}\n` },
  ];
}
