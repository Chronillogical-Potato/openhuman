import { fireEvent, screen } from '@testing-library/react';
import { Route, Routes } from 'react-router-dom';
import { describe, expect, it } from 'vitest';

import { useT } from '../../lib/i18n/I18nContext';
import { memoryErrorMessage } from '../../services/api/memoryApi';
import { renderWithProviders } from '../../test/test-utils';
import MemoryErrorAlert from './MemoryErrorAlert';

/** Renders the alert for `err` the way the memory views do: via memoryErrorMessage. */
function Harness({ err, retry }: { err: unknown; retry?: boolean }) {
  const { t } = useT();
  const message = memoryErrorMessage(err, t);
  return (
    <MemoryErrorAlert message={message} data-testid="alert">
      {retry ? (
        <>
          <span>{message}</span> <button type="button">Retry</button>
        </>
      ) : undefined}
    </MemoryErrorAlert>
  );
}

function renderAt(err: unknown, retry = false) {
  renderWithProviders(
    <Routes>
      <Route path="/" element={<Harness err={err} retry={retry} />} />
      <Route path="/settings/account" element={<div data-testid="billing-page" />} />
    </Routes>
  );
}

const coded = (code: string, message = 'engine detail') =>
  Object.assign(new Error(message), { data: { code, kind: code } });

describe('MemoryErrorAlert', () => {
  it('turns INSUFFICIENT_CREDITS into a top-up prompt, not an error', () => {
    renderAt(coded('INSUFFICIENT_CREDITS', 'insufficient credits: HTTP 402'));
    const alert = screen.getByTestId('alert');
    expect(alert).toHaveAttribute('data-kind', 'out-of-credits');
    expect(alert).toHaveAttribute('data-variant', 'warning');
    expect(alert).toHaveTextContent('Out of credits');
    expect(alert).toHaveTextContent('nothing stored has been lost');
    expect(alert).not.toHaveTextContent('HTTP 402');
  });

  it('sends Top up to the billing page', () => {
    renderAt(coded('INSUFFICIENT_CREDITS'));
    fireEvent.click(screen.getByTestId('memory-top-up'));
    expect(screen.getByTestId('billing-page')).toBeInTheDocument();
  });

  it.each([
    ['UNAVAILABLE', "Memory can't be reached right now"],
    ['ENGINE', 'engine detail'],
    ['UNAUTHORIZED', 'engine detail'],
  ])('keeps the error alert for %s', (code, text) => {
    renderAt(coded(code));
    const alert = screen.getByTestId('alert');
    expect(alert).toHaveAttribute('data-variant', 'destructive');
    expect(alert).toHaveTextContent(text);
    expect(alert).not.toHaveAttribute('data-kind');
    expect(screen.queryByTestId('memory-top-up')).not.toBeInTheDocument();
  });

  it('keeps the error alert for an error with no code', () => {
    renderAt(new Error('something broke'));
    expect(screen.getByTestId('alert')).toHaveAttribute('data-variant', 'destructive');
    expect(screen.queryByTestId('memory-top-up')).not.toBeInTheDocument();
  });

  it('renders the caller content (e.g. a retry) for other errors only', () => {
    renderAt(coded('ENGINE'), true);
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
  });

  it('drops the retry for out of credits: a retry cannot succeed until a top-up', () => {
    renderAt(coded('INSUFFICIENT_CREDITS'), true);
    expect(screen.queryByRole('button', { name: 'Retry' })).not.toBeInTheDocument();
    expect(screen.getByTestId('memory-top-up')).toBeInTheDocument();
  });
});
