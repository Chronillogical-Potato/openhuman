import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { DocumentArtifactCall, MediaGenerationCall } from './MediaAndDocumentCalls';

const baseProps = {
  type: 'tool-call' as const,
  toolCallId: 'call-1',
  argsText: '{}',
  addResult: () => {},
  resume: () => {},
  respondToApproval: () => {},
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
