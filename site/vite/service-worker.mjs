// Emits /sw.js at build time: sw/sw.js prefixed with the cache version and the precache list.
// The list is the shell, the offline page and every file of this build (JS, CSS, wasm), so a
// page that loaded once can show the offline page with no network. The version is the build
// SHA (given explicitly: SITE_BUILD_SHA) plus a digest of what is precached, so a deploy, or
// any change to a precached file, opens a new cache and the activate step drops the old one.
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const siteDir = path.dirname(path.dirname(fileURLToPath(import.meta.url)));

/** Precached besides the build's own files; each is a file in public/ (or the shell, "/"). */
export const SHELL = ['/', '/offline', '/offline.css', '/logo.svg', '/favicon.svg', '/site.webmanifest', '/icon-192.png'];
const PUBLIC_FILE = { '/offline': 'offline.html' };

/** The plugin, for a build of the commit in SITE_BUILD_SHA (required: no build is unversioned). */
export function serviceWorker() {
  const buildSha = process.env.SITE_BUILD_SHA ?? '';
  if (!/^[0-9a-f]{7,40}$/.test(buildSha)) {
    throw new Error(`SITE_BUILD_SHA must be the commit being built (e.g. SITE_BUILD_SHA=$(git rev-parse HEAD)), got ${JSON.stringify(buildSha)}`);
  }
  return {
    name: 'synchronized-work-service-worker',
    apply: 'build',
    generateBundle(_options, bundle) {
      const built = Object.keys(bundle).filter((f) => !f.endsWith('.map') && f !== 'index.html').map((f) => `/${f}`).sort();
      const digest = createHash('sha3-256');
      for (const f of built) digest.update(`${f}\n`);
      for (const url of SHELL.filter((u) => u !== '/')) {
        digest.update(`${url}\n`).update(readFileSync(path.join(siteDir, 'public', PUBLIC_FILE[url] ?? url.slice(1))));
      }
      const source = readFileSync(path.join(siteDir, 'sw', 'sw.js'), 'utf8');
      digest.update(source);
      const version = `${buildSha}-${digest.digest('hex').slice(0, 16)}`;
      this.emitFile({
        type: 'asset',
        fileName: 'sw.js',
        source: `const VERSION = ${JSON.stringify(version)};\nconst PRECACHE = ${JSON.stringify([...SHELL, ...built])};\n${source}`,
      });
    },
  };
}
