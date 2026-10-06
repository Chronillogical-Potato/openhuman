import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../../../../test/test-utils';
import { revealPath } from '../../../../utils/openUrl';
import {
  type AgentPaths,
  openhumanGetAgentPaths,
  openhumanUpdateAgentPaths,
} from '../../../../utils/tauriCommands';
import FilesFolderSection from '../FilesFolderSection';

vi.mock('../../../../utils/tauriCommands', async () => {
  const actual = await vi.importActual<typeof import('../../../../utils/tauriCommands')>(
    '../../../../utils/tauriCommands'
  );
  return { ...actual, openhumanGetAgentPaths: vi.fn(), openhumanUpdateAgentPaths: vi.fn() };
});

vi.mock('../../../../utils/openUrl', () => ({ revealPath: vi.fn() }));

const DEFAULT = '/home/u/OpenHuman/projects/Files';

const paths = (overrides: Partial<AgentPaths> = {}): AgentPaths => ({
  action_dir: '/home/u/OpenHuman/projects',
  workspace_dir: '/home/u/.openhuman/users/u/workspace',
  projects_dir: '/home/u/OpenHuman/projects',
  action_dir_source: 'default',
  files_dir: DEFAULT,
  default_files_dir: DEFAULT,
  files_dir_source: 'default',
  ...overrides,
});

const mockGet = vi.mocked(openhumanGetAgentPaths);
const mockUpdate = vi.mocked(openhumanUpdateAgentPaths);

const input = () => screen.getByTestId('files-folder-input') as HTMLInputElement;

describe('FilesFolderSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGet.mockResolvedValue({ result: paths(), logs: [] });
  });

  it('shows the folder the core reports', async () => {
    renderWithProviders(<FilesFolderSection />);
    await waitFor(() => expect(input().value).toBe(DEFAULT));
    expect(screen.getByText('Files folder')).toBeInTheDocument();
    // Nothing to save until the path changes, and no reset for the default.
    expect(screen.getByTestId('files-folder-save')).toBeDisabled();
    expect(screen.queryByTestId('files-folder-reset')).not.toBeInTheDocument();
  });

  it('saves a new folder and shows what the core stored', async () => {
    mockUpdate.mockResolvedValue({
      result: paths({ files_dir: '/data/Deliverables', files_dir_source: 'override' }),
      logs: [],
    });
    renderWithProviders(<FilesFolderSection />);
    await waitFor(() => expect(input().value).toBe(DEFAULT));

    fireEvent.change(input(), { target: { value: '  /data/Deliverables  ' } });
    fireEvent.click(screen.getByTestId('files-folder-save'));

    await waitFor(() =>
      expect(mockUpdate).toHaveBeenCalledWith({ files_dir: '/data/Deliverables' })
    );
    expect(await screen.findByText('Files folder updated')).toBeInTheDocument();
    expect(input().value).toBe('/data/Deliverables');
    expect(screen.getByTestId('files-folder-reset')).toBeInTheDocument();
  });

  it('Enter saves too', async () => {
    mockUpdate.mockResolvedValue({ result: paths({ files_dir: '/data/F' }), logs: [] });
    renderWithProviders(<FilesFolderSection />);
    await waitFor(() => expect(input().value).toBe(DEFAULT));
    fireEvent.change(input(), { target: { value: '/data/F' } });
    fireEvent.keyDown(input(), { key: 'Enter' });
    await waitFor(() => expect(mockUpdate).toHaveBeenCalledWith({ files_dir: '/data/F' }));
  });

  it('"Use default" clears the override', async () => {
    mockGet.mockResolvedValue({
      result: paths({ files_dir: '/data/Mine', files_dir_source: 'override' }),
      logs: [],
    });
    mockUpdate.mockResolvedValue({ result: paths(), logs: [] });
    renderWithProviders(<FilesFolderSection />);

    fireEvent.click(await screen.findByTestId('files-folder-reset'));

    await waitFor(() => expect(mockUpdate).toHaveBeenCalledWith({ files_dir: '' }));
    await waitFor(() => expect(input().value).toBe(DEFAULT));
    expect(screen.queryByTestId('files-folder-reset')).not.toBeInTheDocument();
  });

  it("shows the core's reason when a folder is refused", async () => {
    mockUpdate.mockRejectedValue(
      new Error('files_dir must not be inside the OpenHuman data folder: /home/u/.openhuman/x')
    );
    renderWithProviders(<FilesFolderSection />);
    await waitFor(() => expect(input().value).toBe(DEFAULT));
    fireEvent.change(input(), { target: { value: '/home/u/.openhuman/x' } });
    fireEvent.click(screen.getByTestId('files-folder-save'));

    expect(
      await screen.findByText(/must not be inside the OpenHuman data folder/)
    ).toBeInTheDocument();
    expect(input().value).toBe('/home/u/.openhuman/x');
  });

  it('says so when the folder cannot be loaded', async () => {
    mockGet.mockRejectedValue(new Error('core down'));
    renderWithProviders(<FilesFolderSection />);
    expect(await screen.findByText('Could not load the files folder.')).toBeInTheDocument();
    expect(input()).toBeDisabled();
  });

  it('Show in folder opens the configured folder', async () => {
    mockGet.mockResolvedValue({
      result: paths({ files_dir: '/data/Mine', files_dir_source: 'override' }),
      logs: [],
    });
    vi.mocked(revealPath).mockResolvedValueOnce(undefined);
    renderWithProviders(<FilesFolderSection />);
    await waitFor(() => expect(input().value).toBe('/data/Mine'));

    fireEvent.click(screen.getByTestId('files-folder-open'));

    await waitFor(() => expect(revealPath).toHaveBeenCalledWith('/data/Mine'));
  });

  it('says so when the folder cannot be opened', async () => {
    vi.mocked(revealPath).mockRejectedValueOnce(new Error('no file manager'));
    renderWithProviders(<FilesFolderSection />);
    await waitFor(() => expect(input().value).toBe(DEFAULT));

    fireEvent.click(screen.getByTestId('files-folder-open'));

    expect(await screen.findByText('Couldn’t open the files folder.')).toBeInTheDocument();
  });
});
