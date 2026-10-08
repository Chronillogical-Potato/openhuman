import { fireEvent, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { useLocation } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { EngineState } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import Memory from '../Memory';

const hoisted = vi.hoisted(() => ({ engineGet: vi.fn(), enginesList: vi.fn() }));

vi.mock('../../services/api/memoryApi', async importOriginal => ({
  ...(await importOriginal<typeof import('../../services/api/memoryApi')>()),
  memoryEngineGet: (...a: unknown[]) => hoisted.engineGet(...a),
  memoryEnginesList: (...a: unknown[]) => hoisted.enginesList(...a),
}));

// The tabs have their own suites; here they only need to say which one rendered.
vi.mock('../../components/memory/MemoryEngineTab', () => ({
  default: () => <div data-testid="stub-engine" />,
}));
vi.mock('../../components/memory/MemoryConversationsTab', () => ({
  default: () => <div data-testid="stub-conversations" />,
}));
// The Files tab has its own suite; here it only reports its props.
vi.mock('../../components/memory/MemoryBrainTab', () => ({
  default: ({ engineLabel, offState }: { engineLabel: string; offState?: ReactNode }) => (
    <div data-testid="stub-brain">
      {engineLabel}
      {offState}
    </div>
  ),
}));

const ON: EngineState = {
  engine: 'tinyhumans',
  has_key: false,
  status: 'ok',
  fetch_modes: ['hybrid'],
};
const OFF: EngineState = {
  engine: null,
  has_key: false,
  status: 'off',
  reason: 'Sign in or add a CortexDB key',
  fetch_modes: [],
};

function Where() {
  const { search } = useLocation();
  return <div data-testid="where">{search}</div>;
}

function renderAt(search: string) {
  return renderWithProviders(
    <>
      <Memory />
      <Where />
    </>,
    { initialEntries: [`/connections${search}`] }
  );
}

beforeEach(() => {
  hoisted.engineGet.mockReset().mockResolvedValue(ON);
  hoisted.enginesList
    .mockReset()
    .mockResolvedValue({
      engines: [{ id: 'tinyhumans', label: 'TinyHumans' }],
      active: 'tinyhumans',
    });
});

describe('Memory page', () => {
  it('renders exactly the three tabs', async () => {
    renderAt('?tab=brain');
    for (const chip of ['engine', 'conversations', 'brain']) {
      expect(await screen.findByTestId(`brain-tab-${chip}`)).toBeInTheDocument();
    }
    for (const gone of ['migration', 'ask', 'explorer', 'learnings', 'background', 'settings']) {
      expect(screen.queryByTestId(`brain-tab-${gone}`)).not.toBeInTheDocument();
    }
    expect(screen.getByTestId('brain-tab-brain')).toHaveTextContent('Files');
  });

  it('defaults to Provider when an engine is active', async () => {
    renderAt('?tab=brain');
    expect(await screen.findByTestId('stub-engine')).toBeInTheDocument();
    expect(screen.queryByTestId('stub-brain')).not.toBeInTheDocument();
  });

  it('defaults to Provider when memory is off', async () => {
    hoisted.engineGet.mockResolvedValue(OFF);
    renderAt('?tab=brain');
    expect(await screen.findByTestId('stub-engine')).toBeInTheDocument();
  });

  it('hands the Files tab the active engine label', async () => {
    renderAt('?tab=brain&brain=brain');
    expect(await screen.findByTestId('stub-brain')).toHaveTextContent('TinyHumans');
  });

  it('passes the off state to the Files tab while memory is off', async () => {
    hoisted.engineGet.mockResolvedValue(OFF);
    renderAt('?tab=brain&brain=brain');
    const brain = await screen.findByTestId('stub-brain');
    expect(brain).toContainElement(screen.getByTestId('memory-off-state'));
  });

  it.each([
    ['conversations', 'stub-conversations'],
    ['brain', 'stub-brain'],
    ['engine', 'stub-engine'],
  ])('opens the %s chip from ?brain=', async (chip, testId) => {
    renderAt(`?tab=brain&brain=${chip}`);
    expect(await screen.findByTestId(testId)).toBeInTheDocument();
  });

  it.each([
    ['migration', 'brain', 'stub-brain'],
    ['ask', 'conversations', 'stub-conversations'],
    ['explorer', 'conversations', 'stub-conversations'],
    ['learnings', 'conversations', 'stub-conversations'],
    ['background', 'conversations', 'stub-conversations'],
    ['settings', 'conversations', 'stub-conversations'],
    ['graph', 'conversations', 'stub-conversations'],
    ['goals', 'conversations', 'stub-conversations'],
    ['context', 'conversations', 'stub-conversations'],
    ['sources', 'brain', 'stub-brain'],
    ['sync', 'brain', 'stub-brain'],
    ['history', 'brain', 'stub-brain'],
    ['documents', 'brain', 'stub-brain'],
  ])('rewrites legacy ?brain=%s to %s', async (legacy, chip, testId) => {
    renderAt(`?tab=brain&brain=${legacy}&view=history`);
    expect(await screen.findByTestId(testId)).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByTestId('where')).toHaveTextContent(`?tab=brain&brain=${chip}`)
    );
    expect(screen.getByTestId('where').textContent).not.toContain('view=');
  });

  it('rewrites an unknown ?brain= value to the Provider chip', async () => {
    renderAt('?tab=brain&brain=bogus');
    expect(await screen.findByTestId('stub-engine')).toBeInTheDocument();
    await waitFor(() =>
      expect(screen.getByTestId('where')).toHaveTextContent('?tab=brain&brain=engine')
    );
    expect(screen.getByTestId('where').textContent).not.toContain('bogus');
  });

  it('switches chips through the URL', async () => {
    renderAt('?tab=brain&brain=conversations');
    await screen.findByTestId('stub-conversations');
    fireEvent.click(screen.getByTestId('brain-tab-brain'));
    expect(await screen.findByTestId('stub-brain')).toBeInTheDocument();
    expect(screen.getByTestId('where')).toHaveTextContent('brain=brain');
  });

  it('shows the off state on Conversations and links to the Provider chip', async () => {
    hoisted.engineGet.mockResolvedValue(OFF);
    renderAt('?tab=brain&brain=conversations');
    expect(await screen.findByTestId('memory-off-state')).toHaveTextContent(
      'Sign in or add a CortexDB key'
    );
    expect(screen.queryByTestId('stub-conversations')).not.toBeInTheDocument();
    fireEvent.click(screen.getByTestId('memory-off-open-engine'));
    expect(await screen.findByTestId('stub-engine')).toBeInTheDocument();
    expect(screen.getByTestId('where')).toHaveTextContent('brain=engine');
  });

  it('treats an unreadable engine as off, says why, and retries', async () => {
    hoisted.engineGet.mockRejectedValueOnce(new Error('core unreachable'));
    renderAt('?tab=brain&brain=conversations');
    expect(await screen.findByTestId('memory-load-error')).toHaveTextContent('core unreachable');
    expect(screen.getByTestId('memory-off-state')).toBeInTheDocument();

    fireEvent.click(screen.getByTestId('memory-load-retry'));
    expect(await screen.findByTestId('stub-conversations')).toBeInTheDocument();
    expect(screen.queryByTestId('memory-load-error')).not.toBeInTheDocument();
  });
});
