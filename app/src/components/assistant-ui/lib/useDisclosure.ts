import { useCallback, useState } from 'react';

/**
 * The user's open/closed choice for a disclosure, remembered by a stable key.
 *
 * Tool cards, delegation cards and reasoning blocks live inside assistant-ui's
 * message tree, which unmounts and remounts them more often than it looks: a
 * thread switch, the virtualised history, a message whose parts change shape.
 * With the choice in local `useState` every remount reset it, so a card the
 * user had opened snapped shut under them. Keyed by something that outlives the
 * component (a tool-call id), the choice survives the remount.
 *
 * Only an explicit toggle is stored. Until the user touches a disclosure it
 * follows `defaultOpen`, which callers derive from state that is the same live
 * and on reload, so a streamed turn and the same turn reopened look alike.
 *
 * The store is module-scoped and deliberately not persisted: the choice matters
 * for as long as the card is on screen in this session, not across launches.
 * It is bounded so a long session cannot grow it without limit.
 */
const MAX_REMEMBERED = 500;
const choices = new Map<string, boolean>();

function remember(key: string, open: boolean): void {
  // Re-insert so the Map's insertion order doubles as recency.
  choices.delete(key);
  choices.set(key, open);
  if (choices.size > MAX_REMEMBERED) {
    const oldest = choices.keys().next().value;
    if (oldest !== undefined) choices.delete(oldest);
  }
}

/** Read a remembered choice. Exposed for tests. */
export function rememberedDisclosure(key: string): boolean | undefined {
  return choices.get(key);
}

/** Forget every remembered choice. Exposed for tests. */
export function resetRememberedDisclosures(): void {
  choices.clear();
}

/**
 * `[open, setOpen]` for a disclosure. With no `key` it is plain local state.
 * The returned `open` is the user's choice when they made one, else
 * `defaultOpen`.
 */
export function useDisclosure(
  key: string | undefined,
  defaultOpen: boolean
): [boolean, (open: boolean) => void] {
  const [choice, setChoice] = useState<boolean | undefined>(() =>
    key === undefined ? undefined : choices.get(key)
  );
  const setOpen = useCallback(
    (open: boolean) => {
      if (key !== undefined) remember(key, open);
      setChoice(open);
    },
    [key]
  );
  return [choice ?? defaultOpen, setOpen];
}
