import { configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type React from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import type { ArtifactSnapshot } from '../../../../store/chatRuntimeSlice';
import { DocumentArtifactCall } from '../MediaAndDocumentCalls';

const THREAD_ID = 'thread-1';
const TOOL_CALL_ID = 'call-doc-1';

vi.mock('../../../../providers/AssistantUiRuntimeProvider', () => ({
  useAuiThreadId: () => THREAD_ID,
}));

const aiRegenerateMock = vi.fn().mockResolvedValue(true);
vi.mock('../../../../services/chatService', () => ({
  aiRegenerate: (...args: unknown[]) => aiRegenerateMock(...args),
}));

function buildStore(artifacts: ArtifactSnapshot[]) {
  return configureStore({
    reducer: { chatRuntime: () => ({ artifactsByThread: { [THREAD_ID]: artifacts } }) },
  });
}

function renderCall(
  props: Partial<React.ComponentProps<typeof DocumentArtifactCall>>,
  artifacts: ArtifactSnapshot[] = []
) {
  return render(
    <Provider store={buildStore(artifacts)}>
      <DocumentArtifactCall
        {...({
          toolCallId: TOOL_CALL_ID,
          type: 'tool-call',
          toolName: 'generate_document',
          args: { title: 'Report' },
          status: { type: 'complete' },
          addResult: vi.fn(),
          resume: vi.fn(),
          respondToApproval: vi.fn(),
          ...props,
        } as unknown as React.ComponentProps<typeof DocumentArtifactCall>)}
      />
    </Provider>
  );
}

describe('DocumentArtifactCall', () => {
  it('renders the settled title/meta when there is no failed snapshot for this call', () => {
    renderCall({ result: { title: 'Report', path: 'a-1/report.docx' } });
    expect(screen.getByText('Report')).toBeInTheDocument();
  });

  it('renders a failed state + Retry when a failed artifact snapshot matches this toolCallId', async () => {
    const artifacts: ArtifactSnapshot[] = [
      {
        artifactId: 'a-1',
        kind: 'document',
        title: 'Report',
        status: 'failed',
        error: 'producer crashed',
        updatedAt: 0,
        toolCallId: TOOL_CALL_ID,
      },
    ];
    renderCall({ result: { title: 'Report' } }, artifacts);

    const retry = screen.getByRole('button');
    await userEvent.click(retry);
    expect(aiRegenerateMock).toHaveBeenCalledWith('a-1', THREAD_ID);
  });

  it('does not show Retry for a failed artifact belonging to a different call', () => {
    const artifacts: ArtifactSnapshot[] = [
      {
        artifactId: 'a-1',
        kind: 'document',
        title: 'Report',
        status: 'failed',
        error: 'producer crashed',
        updatedAt: 0,
        toolCallId: 'some-other-call',
      },
    ];
    renderCall({ result: { title: 'Report' } }, artifacts);
    expect(screen.queryByRole('button')).toBeNull();
  });
});
