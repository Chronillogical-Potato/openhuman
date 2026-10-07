import { fireEvent, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { renderWithProviders } from '../test/test-utils';
import TimezoneSelect from './TimezoneSelect';

const getMock = vi.fn();
const updateMock = vi.fn();
vi.mock('../utils/tauriCommands', () => ({
  openhumanGetUserTimezone: () => getMock(),
  openhumanUpdateUserTimezone: (zone: string | null) => updateMock(zone),
}));

const settings = (timezone: string | null) => ({
  result: { timezone, device: 'Asia/Kolkata', effective: timezone ?? 'Asia/Kolkata' },
  logs: [],
});

function render() {
  return renderWithProviders(<TimezoneSelect />, { preloadedState: { locale: { current: 'en' } } });
}

describe('TimezoneSelect', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('shows the saved zone, and offers the device zone by name', async () => {
    getMock.mockResolvedValue(settings('Europe/Berlin'));
    render();
    const select = await screen.findByTestId('timezone-select');
    await waitFor(() => expect((select as HTMLSelectElement).value).toBe('Europe/Berlin'));
    expect(
      screen.getByRole('option', { name: 'Use device time zone (Asia/Kolkata)' })
    ).toBeTruthy();
  });

  it('saves a chosen zone and re-reads the setting', async () => {
    getMock
      .mockResolvedValueOnce(settings(null))
      .mockResolvedValueOnce(settings('America/New_York'));
    updateMock.mockResolvedValue({ result: {}, logs: [] });
    render();
    const select = (await screen.findByTestId('timezone-select')) as HTMLSelectElement;
    await waitFor(() => expect(select.disabled).toBe(false));
    expect(select.value).toBe('');

    fireEvent.change(select, { target: { value: 'America/New_York' } });
    await waitFor(() => expect(updateMock).toHaveBeenCalledWith('America/New_York'));
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(2));
    expect(select.value).toBe('America/New_York');
  });

  it('following the device sends null', async () => {
    getMock.mockResolvedValue(settings('Europe/Berlin'));
    updateMock.mockResolvedValue({ result: {}, logs: [] });
    render();
    const select = (await screen.findByTestId('timezone-select')) as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe('Europe/Berlin'));
    fireEvent.change(select, { target: { value: '' } });
    await waitFor(() => expect(updateMock).toHaveBeenCalledWith(null));
  });

  it('a failed save restores the previous zone and says so', async () => {
    getMock.mockResolvedValue(settings('Europe/Berlin'));
    updateMock.mockRejectedValue(new Error('core offline'));
    render();
    const select = (await screen.findByTestId('timezone-select')) as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe('Europe/Berlin'));
    fireEvent.change(select, { target: { value: 'Asia/Tokyo' } });
    expect(await screen.findByTestId('timezone-error')).toBeTruthy();
    expect(select.value).toBe('Europe/Berlin');
  });

  it('a failed re-read after a successful save keeps the new zone, without an error', async () => {
    getMock
      .mockResolvedValueOnce(settings('Europe/Berlin'))
      .mockRejectedValueOnce(new Error('core offline'));
    updateMock.mockResolvedValue({ result: {}, logs: [] });
    render();
    const select = (await screen.findByTestId('timezone-select')) as HTMLSelectElement;
    await waitFor(() => expect(select.value).toBe('Europe/Berlin'));
    fireEvent.change(select, { target: { value: 'Asia/Tokyo' } });
    await waitFor(() => expect(getMock).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(select.disabled).toBe(false));
    expect(select.value).toBe('Asia/Tokyo');
    expect(screen.queryByTestId('timezone-error')).toBeNull();
  });

  it('is disabled while a save is in flight, so picks cannot overlap', async () => {
    getMock.mockResolvedValue(settings('Europe/Berlin'));
    let finish: (value: unknown) => void = () => {};
    updateMock.mockReturnValue(new Promise(resolve => (finish = resolve)));
    render();
    const select = (await screen.findByTestId('timezone-select')) as HTMLSelectElement;
    await waitFor(() => expect(select.disabled).toBe(false));
    fireEvent.change(select, { target: { value: 'Asia/Tokyo' } });
    await waitFor(() => expect(select.disabled).toBe(true));
    finish({ result: {}, logs: [] });
    await waitFor(() => expect(select.disabled).toBe(false));
    expect(updateMock).toHaveBeenCalledTimes(1);
  });

  it('a core that cannot be read leaves the picker disabled, not broken', async () => {
    getMock.mockRejectedValue(new Error('core offline'));
    render();
    expect(await screen.findByTestId('timezone-error')).toBeTruthy();
    expect((screen.getByTestId('timezone-select') as HTMLSelectElement).disabled).toBe(true);
  });
});
