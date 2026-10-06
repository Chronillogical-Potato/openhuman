import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import Field from './Field';

describe('Field', () => {
  it('keeps the row structure when there is no error', () => {
    const { container } = render(
      <Field htmlFor="a" label="Name" control={<input id="a" />} data-testid="f" />
    );
    const root = screen.getByTestId('f');
    expect(root.className).toContain('flex items-center justify-between gap-4 px-4 py-3');
    expect(root.className).not.toContain('flex-wrap');
    expect(container.querySelector('[data-slot="field-error"]')).toBeNull();
    expect(screen.getByRole('textbox')).not.toHaveAttribute('aria-invalid');
    expect(screen.getByRole('textbox')).not.toHaveAttribute('aria-describedby');
  });

  it('renders the error and wires it to the control', () => {
    render(<Field htmlFor="a" label="Name" error="Required" control={<input id="a" />} />);
    const msg = screen.getByText('Required');
    const input = screen.getByRole('textbox');
    expect(msg.id).toBeTruthy();
    expect(input).toHaveAttribute('aria-describedby', msg.id);
    expect(input).toHaveAttribute('aria-invalid', 'true');
  });

  it('merges an existing aria-describedby', () => {
    render(<Field error="Bad" control={<input aria-describedby="hint" />} />);
    const input = screen.getByRole('textbox');
    const id = screen.getByText('Bad').id;
    expect(input.getAttribute('aria-describedby')).toBe(`hint ${id}`);
  });
});
