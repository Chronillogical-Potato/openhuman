import { cn } from '../../lib/cn';
import { CheckIcon } from './icons';

export interface StepperProps {
  /** Ordered labels for each step. */
  labels: string[];
  /** Zero-based index of the step that is currently active. */
  activeIndex: number;
  /** Accessible name for the step list. Required: a primitive owns no copy. */
  ariaLabel: string;
  className?: string;
  'data-testid'?: string;
}

/**
 * Horizontal step indicator — a dot per step joined by connectors, with three
 * states: completed (sage, check), active (primary, `aria-current="step"`) and
 * upcoming (outlined). Promoted out of the onboarding wizard.
 */
const Stepper = ({
  labels,
  activeIndex,
  ariaLabel,
  className,
  'data-testid': testId,
}: StepperProps) => (
  <ol
    role="list"
    data-slot="stepper"
    data-testid={testId}
    aria-label={ariaLabel}
    className={cn('flex w-full items-start justify-between', className)}>
    {labels.map((label, idx) => {
      const completed = idx < activeIndex;
      const active = idx === activeIndex;
      const isLast = idx === labels.length - 1;

      const dotClasses = completed
        ? 'bg-sage-500 border-sage-500 text-content-inverted'
        : active
          ? 'bg-primary-500 border-primary-500 text-content-inverted'
          : 'bg-surface border-line-strong text-content-faint';

      const labelClasses = completed
        ? 'text-sage-700 dark:text-sage-300'
        : active
          ? 'text-content font-semibold'
          : 'text-content-faint';

      const connectorClasses = completed ? 'bg-sage-500' : 'bg-surface-strong';

      return (
        <li
          key={label}
          data-slot="stepper-item"
          aria-current={active ? 'step' : undefined}
          className="relative flex flex-1 flex-col items-center">
          <div className="flex w-full items-center">
            <div
              aria-hidden
              className={cn('h-0.5 flex-1', idx === 0 ? 'opacity-0' : connectorClasses)}
            />
            <div
              className={cn(
                'flex h-6 w-6 flex-none items-center justify-center rounded-full border-2 text-[10px] font-semibold',
                dotClasses
              )}>
              {completed ? <CheckIcon className="h-3 w-3" aria-hidden /> : idx + 1}
            </div>
            <div
              aria-hidden
              className={cn('h-0.5 flex-1', isLast ? 'opacity-0' : connectorClasses)}
            />
          </div>
          <span className={cn('mt-2 text-[11px] leading-tight text-center', labelClasses)}>
            {label}
          </span>
        </li>
      );
    })}
  </ol>
);

export default Stepper;
