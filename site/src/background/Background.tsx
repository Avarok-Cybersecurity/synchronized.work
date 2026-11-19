import { useEffect, useRef } from 'react';
import { PERIOD_S, draw, grow, type Palette } from './scene';

// Decorative only (aria-hidden, no pointer events). Reduced motion draws one still frame; a
// hidden tab or an off-screen canvas pauses; a device that cannot draw a frame in 12 ms six
// times running gets the still frame too. Colours are the theme's --bg-* tokens.
function palette(): Palette {
  const s = getComputedStyle(document.documentElement);
  const get = (name: string) => {
    const v = s.getPropertyValue(name).trim();
    if (!v) throw new Error(`${name} is not set`);
    return v;
  };
  return { ink: get('--bg-ink'), primary: get('--bg-primary'), secondary: get('--bg-secondary') };
}

export function Background() {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    const reduced = matchMedia('(prefers-reduced-motion: reduce)');
    let seed = (crypto.getRandomValues(new Uint32Array(1))[0] ?? 1) >>> 0;
    let edges = grow(seed);
    let raf = 0;
    let epoch = performance.now();
    let slow = 0;
    let pal = palette();
    const size = () => {
      const dpr = Math.min(devicePixelRatio || 1, 1.5);
      canvas.width = Math.max(1, Math.round(canvas.clientWidth * dpr));
      canvas.height = Math.max(1, Math.round(canvas.clientHeight * dpr));
    };
    const still = () => {
      canvas.dataset.motion = 'still';
      draw(ctx, canvas.width, canvas.height, edges, 0, pal, true);
    };
    const tick = (now: number) => {
      raf = 0;
      let t = (now - epoch) / 1000;
      if (t >= PERIOD_S) {
        epoch = now;
        t = 0;
        seed = (seed * 1103515245 + 12345) >>> 0;
        edges = grow(seed);
      }
      const began = performance.now();
      draw(ctx, canvas.width, canvas.height, edges, t, pal, false);
      slow = performance.now() - began > 12 ? slow + 1 : 0;
      if (slow >= 6) return still();
      raf = requestAnimationFrame(tick);
    };
    const play = () => {
      if (reduced.matches) return still();
      canvas.dataset.motion = 'animated';
      if (!raf && !document.hidden) raf = requestAnimationFrame(tick);
    };
    const pause = () => {
      cancelAnimationFrame(raf);
      raf = 0;
    };
    const onVisibility = () => (document.hidden ? pause() : play());
    const onResize = () => {
      size();
      if (!raf) still();
    };
    const onScheme = () => {
      pal = palette();
      if (!raf) still();
    };
    const onReduced = () => (reduced.matches ? (pause(), still()) : play());
    const themeObserver = new MutationObserver(onScheme);
    themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
    const scheme = matchMedia('(prefers-color-scheme: dark)');
    size();
    play();
    addEventListener('resize', onResize);
    document.addEventListener('visibilitychange', onVisibility);
    reduced.addEventListener('change', onReduced);
    scheme.addEventListener('change', onScheme);
    return () => {
      pause();
      themeObserver.disconnect();
      removeEventListener('resize', onResize);
      document.removeEventListener('visibilitychange', onVisibility);
      reduced.removeEventListener('change', onReduced);
      scheme.removeEventListener('change', onScheme);
    };
  }, []);
  return <canvas ref={ref} className="bg-canvas" aria-hidden="true" />;
}
