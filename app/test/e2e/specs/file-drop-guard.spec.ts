import { expect } from '@wdio/globals';

import { waitForApp } from '../helpers/app-helpers';
import { clickByTitle } from '../helpers/chat-harness';
import { dispatchFileDrop, waitForDataSlot, waitForTestId } from '../helpers/element-helpers';
import { resetApp } from '../helpers/reset-app';
import { navigateViaHash } from '../helpers/shared-flows';
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
    const sidebar = await waitForTestId('root-shell-sidebar');
    const result = await dispatchFileDrop(sidebar, {
      name: 'unclaimed.txt',
      type: 'text/plain',
      contents: 'dropped from e2e',
    });

    expect(result).toEqual({ dragOverPrevented: true, dropPrevented: true, fileCount: 1 });
  });

  it('claims a real file drop over the open chat thread', async () => {
    await navigateViaHash('/chat');
    expect(await clickByTitle('New thread', 8_000)).toBe(true);

    const thread = await waitForDataSlot('aui_thread-viewport');
    const result = await dispatchFileDrop(thread, {
      name: 'thread-drop.txt',
      type: 'text/plain',
      contents: 'dropped onto the transcript',
    });

    expect(result).toEqual({ dragOverPrevented: true, dropPrevented: true, fileCount: 1 });
  });
});
