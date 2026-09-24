import { configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type React from 'react';
import { Provider } from 'react-redux';
import { describe, expect, it, vi } from 'vitest';

import type { ArtifactSnapshot } from '../../../store/chatRuntimeSlice';
import { DocumentArtifactCall, MediaGenerationCall } from './MediaAndDocumentCalls';

const THREAD_ID = 'thread-1';

vi.mock('../../../providers/AssistantUiRuntimeProvider', () => ({
  useAuiThreadId: () => THREAD_ID,
}));

const aiRegenerateMock = vi.fn().mockResolvedValue(true);
vi.mock('../../../services/chatService', () => ({
  aiRegenerate: (...args: unknown[]) => aiRegenerateMock(...args),
}));

function withStore(node: React.ReactElement, artifacts: ArtifactSnapshot[] = []) {
  const store = configureStore({
    reducer: { chatRuntime: () => ({ artifactsByThread: { [THREAD_ID]: artifacts } }) },
  });
  return <Provider store={store}>{node}</Provider>;
}

const baseProps = {
  type: 'tool-call' as const,
  toolCallId: 'call-1',
  argsText: '{}',
  addResult: () => {},
  resume: () => {},
  respondToApproval: async () => {},
};

describe('MediaGenerationCall', () => {
  it('shows the image-generation placeholder while the tool runs', () => {
    render(
      <MediaGenerationCall
        {...baseProps}
        toolName="media_generate_image"
        args={{ prompt: 'a red fox in snow' } as never}
        result={undefined}
        status={{ type: 'running' }}
      />
    );

    expect(screen.getByText('Generating')).toBeInTheDocument();
  });

  it('renders one image per produced artifact once the tool completes', () => {
    render(
      <MediaGenerationCall
        {...baseProps}
        toolName="media_generate_image"
        args={{ prompt: 'a red fox in snow' } as never}
        result={
          {
            artifacts: [
              { type: 'image', source_url: 'https://example.com/fox.png', artifact_id: 'art-1' },
            ],
          } as never
        }
        status={{ type: 'complete' }}
      />
    );

    expect(screen.getByTestId('assistant-ui-media-generation-result')).toBeInTheDocument();
  });
});

describe('DocumentArtifactCall', () => {
  it('shows the artifact card generating while the tool runs', () => {
    render(
      withStore(
        <DocumentArtifactCall
          {...baseProps}
          toolName="generate_document"
          args={{ title: 'Q3 report' } as never}
          result={undefined}
          status={{ type: 'running' }}
        />
      )
    );

    expect(screen.getByText('Q3 report')).toBeInTheDocument();
    expect(screen.getByText('Writing')).toBeInTheDocument();
  });

  it('shows the settled artifact once generation completes', () => {
    render(
      withStore(
        <DocumentArtifactCall
          {...baseProps}
          toolName="generate_presentation"
          args={{} as never}
          result={{ title: 'Board deck', path: '/artifacts/board-deck.pptx' } as never}
          status={{ type: 'complete' }}
        />
      )
    );

    expect(screen.getByText('Board deck')).toBeInTheDocument();
    expect(screen.getByText('/artifacts/board-deck.pptx')).toBeInTheDocument();
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
        toolCallId: 'call-1',
      },
    ];
    render(
      withStore(
        <DocumentArtifactCall
          {...baseProps}
          toolName="generate_document"
          args={{ title: 'Report' } as never}
          result={{ title: 'Report' } as never}
          status={{ type: 'complete' }}
        />,
        artifacts
      )
    );

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
    render(
      withStore(
        <DocumentArtifactCall
          {...baseProps}
          toolName="generate_document"
          args={{ title: 'Report' } as never}
          result={{ title: 'Report' } as never}
          status={{ type: 'complete' }}
        />,
        artifacts
      )
    );
    expect(screen.queryByRole('button')).toBeNull();
  });
});
