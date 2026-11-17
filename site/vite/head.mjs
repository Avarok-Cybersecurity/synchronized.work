// The <head> identity tags, derived from brand/brand.json (SSOT with the manifest and icons
// that scripts/brand.mjs writes). Pure; vite.config.ts injects the result into index.html.
// Attributes only: nothing inline, so the Worker's CSP holds.

/** @param {{ origin: string, name: string, tagline: string, description: string, ogImageAlt: string, colors: Record<string, string> }} brand */
export function headTags(brand) {
  const url = `${brand.origin}/`;
  const title = `${brand.name} — ${brand.tagline}`;
  const image = `${brand.origin}/og-image.png`;
  const meta = (attrs) => ({ tag: 'meta', attrs });
  const link = (attrs) => ({ tag: 'link', attrs });
  const og = (property, content) => meta({ property, content });
  const tw = (name, content) => meta({ name, content });
  return [
    { tag: 'title', children: title },
    meta({ name: 'description', content: brand.description }),
    link({ rel: 'canonical', href: url }),
    link({ rel: 'icon', href: '/favicon.ico', sizes: '16x16 32x32 48x48' }),
    link({ rel: 'icon', href: '/favicon.svg', type: 'image/svg+xml' }),
    link({ rel: 'apple-touch-icon', href: '/apple-touch-icon.png' }),
    link({ rel: 'manifest', href: '/site.webmanifest' }),
    meta({ name: 'theme-color', media: '(prefers-color-scheme: light)', content: brand.colors.pageLight }),
    meta({ name: 'theme-color', media: '(prefers-color-scheme: dark)', content: brand.colors.pageDark }),
    og('og:type', 'website'),
    og('og:site_name', brand.name),
    og('og:url', url),
    og('og:title', title),
    og('og:description', brand.description),
    og('og:image', image),
    og('og:image:type', 'image/png'),
    og('og:image:width', '1200'),
    og('og:image:height', '630'),
    og('og:image:alt', brand.ogImageAlt),
    tw('twitter:card', 'summary_large_image'),
    tw('twitter:title', title),
    tw('twitter:description', brand.description),
    tw('twitter:image', image),
    tw('twitter:image:alt', brand.ogImageAlt),
  ].map((t) => ({ ...t, injectTo: 'head' }));
}
