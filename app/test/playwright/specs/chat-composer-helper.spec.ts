import { expect, test } from '@playwright/test';

import { clearChatComposer, composerText, replaceChatComposerText } from '../helpers/chat-composer';

// Exercise the keyboard helper in Chromium with restored text in both surfaces.
// The product specs below use the same helper against the shipped composer.
test.describe('Composer draft isolation helper', () => {
  for (const surface of ['textarea', 'contenteditable'] as const) {
    test(`replaces a restored ${surface} draft and a second prompt`, async ({ page }) => {
      await page.setContent(
        surface === 'textarea'
          ? '<textarea data-testid="composer">draft from the previous test</textarea>'
          : '<div contenteditable="true" data-testid="composer">draft from the previous test</div>'
      );
      const input = page.getByTestId('composer');
      expect(await composerText(input)).toBe('draft from the previous test');

      await replaceChatComposerText(input, 'first prompt');
      expect(await composerText(input)).toBe('first prompt');
      await replaceChatComposerText(input, 'second prompt');
      expect(await composerText(input)).toBe('second prompt');

      await clearChatComposer(input);
      expect(await composerText(input)).toBe('');
    });
  }

  test('clears a multiline restored draft before an empty prompt', async ({ page }) => {
    await page.setContent(
      '<div contenteditable="true" data-testid="composer"><p>old</p><p>draft</p></div>'
    );
    const input = page.getByTestId('composer');
    await replaceChatComposerText(input, '');
    expect(await composerText(input)).toBe('');
    await expect(input).toBeFocused();
  });
});
