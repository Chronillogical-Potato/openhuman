import { configureStore } from '@reduxjs/toolkit';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
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
      <DocumentArtifactCall
        {...baseProps}
        toolName="generate_document"
        args={{ title: 'Q3 report' } as never}
        result={undefined}
        status={{ type: 'running' }}
      />
    );

    expect(screen.getByText('Q3 report')).toBeInTheDocument();
    expect(screen.getByText('Writing')).toBeInTheDocument();
  });

  it('shows the settled artifact once generation completes', () => {
    render(
      <DocumentArtifactCall
        {...baseProps}
        toolName="generate_presentation"
        args={{} as never}
        result={{ title: 'Board deck', path: '/artifacts/board-deck.pptx' } as never}
        status={{ type: 'complete' }}
      />
    );

    expect(screen.getByText('Board deck')).toBeInTheDocument();
    expect(screen.getByText('/artifacts/board-deck.pptx')).toBeInTheDocument();
  });
});
