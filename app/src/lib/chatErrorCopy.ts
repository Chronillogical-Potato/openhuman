/**
 * Localized render of a `chat_error` event's user-facing copy.
 *
 * The core always sends `message` as finished English (older UIs, the CLI, the
 * TUI and embedders render it verbatim). It also sends `copy_key`
 * (`chat_error.<class>`, one per row of the core's failure-copy table) and
 * `copy_params` (`retry_after_secs`, `provider`, `detail`). When the key is
 * one this build knows, the text is composed here in the active locale:
 *
 *   <translated copy> [<retry hint>] [\n\n> <provider detail>]
 *
 * which is the same shape the core builds `message` in. An unknown or missing
 * key falls back to `message`, so a newer core never shows a raw key.
 */

/** Values the core attaches to a `chat_error` for the translated copy. */
export interface ChatErrorCopyParams {
  retry_after_secs?: number;
  provider?: string;
  /** Sanitized provider error quoted under the copy; never translated. */
  detail?: string;
}

export interface ChatErrorCopySource {
  message: string;
  copy_key?: string;
  copy_params?: ChatErrorCopyParams;
}

type Translate = (key: string, fallback?: string) => string;

// A string no translation file contains: `t(key, MISSING)` returning it means
// the key is not defined in the active locale or in English.
const MISSING = '\u0000chat-error-copy-missing';

function retryHint(secs: number, t: Translate): string {
  if (secs === 0) return t('chat_error.retryHint.immediately');
  if (secs === 1) return t('chat_error.retryHint.oneSecond');
  if (secs < 90) return t('chat_error.retryHint.seconds').replace('{n}', String(secs));
  // Round up: never tell the user to retry sooner than the upstream allows.
  const mins = Math.floor(secs / 60) + (secs % 60 !== 0 ? 1 : 0);
  return mins === 1
    ? t('chat_error.retryHint.aboutMinute')
    : t('chat_error.retryHint.aboutMinutes').replace('{n}', String(mins));
}

/** The text to show for `event`: translated when its key is known, else `message`. */
export function chatErrorCopyText(event: ChatErrorCopySource, t: Translate): string {
  const key = event.copy_key;
  if (!key || !key.startsWith('chat_error.')) return event.message;
  const base = t(key, MISSING);
  if (base === MISSING) return event.message;

  const params = event.copy_params;
  let text = base;
  const secs = params?.retry_after_secs;
  if (typeof secs === 'number' && Number.isFinite(secs) && secs >= 0) {
    text = `${text} ${retryHint(Math.floor(secs), t)}`;
  }
  if (typeof params?.detail === 'string' && params.detail.length > 0) {
    text = `${text}\n\n> ${params.detail}`;
  }
  return text;
}
