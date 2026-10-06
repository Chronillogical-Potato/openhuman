import { type ComponentPropsWithRef } from 'react';

import { cn } from '../../lib/cn';

export type SkeletonProps = ComponentPropsWithRef<'div'>;

/**
 * A loading placeholder block. Size it with `className` (`h-4 w-32`) or
 * `style` (`{ width, height }`).
 *
 * MOTION. The pulse is gated in CSS with `motion-safe:`, so a user who asks
 * for reduced motion gets a static block — no JS media query involved.
 *
 * A11Y. It conveys nothing to a screen reader, so it is `aria-hidden` by
 * default. Announce loading state on the surrounding region (`aria-busy`, a
 * status message); a caller can still override `aria-hidden` explicitly.
 */
const Skeleton = ({ className, ...rest }: SkeletonProps) => (
  <div
    data-slot="skeleton"
    aria-hidden="true"
    className={cn('rounded-md bg-surface-strong motion-safe:animate-pulse', className)}
    {...rest}
  />
);

export default Skeleton;
