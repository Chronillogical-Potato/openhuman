import { expect, type Locator } from '@playwright/test';

/** Read either the Lexical surface or a textarea composer. */
export function composerText(input: Locator): Promise<string> {
  return input.evaluate(node => {
    const field = node as HTMLTextAreaElement;
    return typeof field.value === 'string' ? field.value : (node.textContent ?? '');
  });
}

/** Clear the restored draft through the same keyboard events a user sends. */
export async function clearChatComposer(input: Locator): Promise<void> {
  await input.click();
  await input.press('ControlOrMeta+a');
  await input.press('Delete');
  await expect.poll(() => composerText(input), { timeout: 15_000 }).toBe('');
}

/** Start a test with exactly its prompt, including when a draft was restored. */
export async function replaceChatComposerText(input: Locator, text: string): Promise<void> {
  await clearChatComposer(input);
  await input.pressSequentially(text);
  await expect.poll(() => composerText(input), { timeout: 15_000 }).toBe(text);
}
