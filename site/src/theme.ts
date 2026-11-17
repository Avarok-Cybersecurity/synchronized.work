// Light, dark, or the system's choice. Only the person's explicit choice is stored, and only
// as a convenience: unreadable storage means "follow the system".
export type ThemeChoice = 'light' | 'dark' | 'system';
const KEY = 'sw.theme';

function stored(): ThemeChoice {
  try {
    const v = localStorage.getItem(KEY);
    return v === 'light' || v === 'dark' ? v : 'system';
  } catch {
    return 'system';
  }
}

export function apply(choice: ThemeChoice): void {
  if (choice === 'system') document.documentElement.removeAttribute('data-theme');
  else document.documentElement.setAttribute('data-theme', choice);
}

export function applyStoredTheme(): void {
  apply(stored());
}

export function effectiveTheme(): 'light' | 'dark' {
  const set = document.documentElement.getAttribute('data-theme');
  if (set === 'light' || set === 'dark') return set;
  return matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function choose(choice: ThemeChoice): void {
  apply(choice);
  try {
    if (choice === 'system') localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, choice);
  } catch {
    // The choice still applies to this page.
  }
}
