// Starts "Sign in with Citadel" (sign-in contract §5): a fresh verifier and state from web
// crypto, kept in sessionStorage for the return, and the issuer link from the Rust core.
import { signInLink } from './wasm/core';

export const PENDING_KEY = 'sw.sign-in.pending';
const STATE_BYTES = 16;

export class SignInUnavailable extends Error {}

const b64url = (bytes: Uint8Array): string =>
  btoa(String.fromCharCode(...bytes)).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

export async function startSignIn(): Promise<string> {
  const verifier = crypto.getRandomValues(new Uint8Array(32));
  const state = crypto.getRandomValues(new Uint8Array(STATE_BYTES));
  const link = await signInLink(verifier, state);
  try {
    sessionStorage.setItem(PENDING_KEY, JSON.stringify({ verifier: b64url(verifier), state: b64url(state) }));
  } catch {
    throw new SignInUnavailable('This browser blocks session storage, which sign-in needs to finish.');
  }
  return link;
}

/** The return page's cleanup: the pending attempt is dropped, whatever happened. */
export function forgetPendingSignIn(): void {
  try {
    sessionStorage.removeItem(PENDING_KEY);
  } catch {
    // Storage that cannot be read holds no pending attempt either.
  }
}
