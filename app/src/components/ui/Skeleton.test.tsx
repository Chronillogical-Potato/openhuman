import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import Skeleton from './Skeleton';

describe('Skeleton', () => {
  it('renders a hidden placeholder', () => {
    const { container } = render(<Skeleton />);
    const el = container.querySelector('[data-slot="skeleton"]');
    expect(el).not.toBeNull();
    expect(el).toHaveAttribute('aria-hidden', 'true');
  });

  it('only animates when motion is allowed', () => {
    const { container } = render(<Skeleton className="h-4 w-32" />);
    const el = container.firstElementChild as HTMLElement;
    expect(el.className).toContain('motion-safe:animate-pulse');
    expect(el.className).not.toMatch(/(^|\s)animate-pulse/);
    expect(el.className).toContain('h-4');
  });

  it('passes style through', () => {
    const { container } = render(<Skeleton style={{ width: 10 }} />);
    expect((container.firstElementChild as HTMLElement).style.width).toBe('10px');
  });
});
