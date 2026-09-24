import { browser, expect } from '@wdio/globals';

import { waitForApp } from '../helpers/app-helpers';
import { dispatchFileDrop, waitForTestId } from '../helpers/element-helpers';
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
    const sidebar = await waitForTestId('root-shell-sidebar');
    const result = await dispatchFileDrop(sidebar, {
      name: 'unclaimed.txt',
      type: 'text/plain',
      contents: 'dropped from e2e',
    });

    expect(result).toEqual({ dragOverPrevented: true, dropPrevented: true, fileCount: 1 });
    expect(await browser.getUrl()).toBe(urlBeforeDrop);
  });
});
