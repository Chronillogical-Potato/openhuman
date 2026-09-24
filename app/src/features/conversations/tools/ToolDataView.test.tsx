import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { ToolDataView } from './ToolDataView';

describe('ToolDataView', () => {
  it('renders a uniform array of flat objects through the data-table element', () => {
    render(
      <ToolDataView
        value={[
          { name: 'alpha', context: '128k', cost: '$0.02' },
          { name: 'beta', context: '32k', cost: '$0.01' },
        ]}
      />
    );

    expect(screen.getByText('alpha')).toBeInTheDocument();
    expect(screen.getByText('beta')).toBeInTheDocument();
    expect(screen.getByText('128k')).toBeInTheDocument();
    expect(screen.getByText('Name')).toBeInTheDocument();
  });

  it('falls back to the generic list for a mixed-shape array', () => {
    render(<ToolDataView value={[{ name: 'alpha' }, { other: 'beta' }]} />);

    expect(screen.getByText('alpha')).toBeInTheDocument();
    expect(screen.getByText('beta')).toBeInTheDocument();
  });

  it('renders a plain object as a definition list', () => {
    render(<ToolDataView value={{ status: 'ok' }} />);

    expect(screen.getByText('Status')).toBeInTheDocument();
    expect(screen.getByText('ok')).toBeInTheDocument();
  });
});
