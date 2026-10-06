// @ts-nocheck
import { randomUUID } from 'node:crypto';
import { promises as fs } from 'node:fs';
import path from 'node:path';

import { waitForApp } from '../helpers/app-helpers';
import { callOpenhumanRpc } from '../helpers/core-rpc';
import { resetApp } from '../helpers/reset-app';
import { startMockServer, stopMockServer } from '../mock-server';

const USER_ID = 'e2e-tool-filesystem';
const PERSONA_FILE = 'SOUL.md';
const TEST_CONTENT = 'Filesystem RPC E2E canary';

function workspaceDir(): string {
  const workspace = process.env.OPENHUMAN_WORKSPACE;
  if (!workspace) throw new Error('OPENHUMAN_WORKSPACE is required for path escape assertions');
  return workspace;
}

describe('Workspace persona files — read, write, and path restriction', () => {
  before(async function beforeSuite() {
    this.timeout(90_000);
    await startMockServer();
    await waitForApp();
    await resetApp(USER_ID);
  });

  after(async () => {
    // Avoid leaving the user's persona file modified if a test fails midway.
    await callOpenhumanRpc('openhuman.workspace_file_reset', { filename: PERSONA_FILE });
    await stopMockServer();
  });

  it('writes and reads an allowlisted workspace persona file', async () => {
    const write = await callOpenhumanRpc('openhuman.workspace_file_write', {
      filename: PERSONA_FILE,
      contents: TEST_CONTENT,
    });
    expect(write.ok).toBe(true);
    expect(write.result?.filename).toBe(PERSONA_FILE);
    expect(write.result?.contents).toBe(TEST_CONTENT);

    const read = await callOpenhumanRpc('openhuman.workspace_file_read', {
      filename: PERSONA_FILE,
    });
    expect(read.ok).toBe(true);
    expect(read.result?.contents).toBe(TEST_CONTENT);
  });

  it('rejects traversal and absolute paths without writing outside the workspace', async () => {
    const escapedFile = path.resolve(workspaceDir(), '..', `e2e-escape-${randomUUID()}.txt`);
    const traversalName = path.relative(workspaceDir(), escapedFile);
    for (const filename of [traversalName, escapedFile]) {
      const result = await callOpenhumanRpc('openhuman.workspace_file_write', {
        filename,
        contents: 'must not be written',
      });
      expect(result.ok).toBe(false);
      expect(result.error?.toLowerCase()).toMatch(/editable|allow|invalid|not found|unknown/);

      const read = await callOpenhumanRpc('openhuman.workspace_file_read', { filename });
      expect(read.ok).toBe(false);
    }

    await expect(fs.readFile(escapedFile, 'utf8')).rejects.toMatchObject({ code: 'ENOENT' });
  });
});
