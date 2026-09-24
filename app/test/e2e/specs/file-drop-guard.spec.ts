// @ts-nocheck
import { browser, expect } from '@wdio/globals';

import { waitForApp } from '../helpers/app-helpers';
import { resetApp } from '../helpers/reset-app';
import { startMockServer, stopMockServer } from '../mock-server';

const USER_ID = 'e2e-file-drop-guard';

describe('File drop guard', () => {
  before(async () => {
    await startMockServer();
    await waitForApp();
    await resetApp(USER_ID);
  });

  after(async () => {
    await stopMockServer();
  });

  it('refuses an unclaimed file drop on the sidebar without navigating the app', async () => {
    const urlBeforeDrop = await browser.getUrl();
    const result = await browser.execute(() => {
      const target = document.querySelector('[data-testid="root-shell-sidebar"]');
      if (!target) return null;

      const dispatchFileEvent = (type: 'dragover' | 'drop') => {
        const event = new Event(type, { bubbles: true, cancelable: true }) as DragEvent;
        Object.defineProperty(event, 'dataTransfer', {
          value: { types: ['Files'], dropEffect: 'copy' },
        });
        target.dispatchEvent(event);
        return event.defaultPrevented;
      };

      return {
        dragOverPrevented: dispatchFileEvent('dragover'),
        dropPrevented: dispatchFileEvent('drop'),
      };
    });

    expect(result).toEqual({ dragOverPrevented: true, dropPrevented: true });
    expect(await browser.getUrl()).toBe(urlBeforeDrop);
  });
});
