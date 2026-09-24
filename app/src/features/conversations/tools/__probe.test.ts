import { describe, expect, it } from 'vitest';

import { describeToolCall, toolLabel } from './toolPresentation';

describe('probe', () => {
  it('prints labels', () => {
    const a = describeToolCall({
      name: 'GMAIL_FETCH_EMAILS',
      args: JSON.stringify({ query: 'from:broker' }),
      status: 'success',
    });
    console.log('GMAIL_FETCH_EMAILS ->', JSON.stringify(a), toolLabel(a));

    const b = describeToolCall({
      name: 'memory_hybrid_search',
      args: JSON.stringify({ query: 'apple stock' }),
      status: 'success',
    });
    console.log('memory_hybrid_search ->', JSON.stringify(b), toolLabel(b));
    expect({ a: JSON.stringify(a), aLabel: toolLabel(a), b: JSON.stringify(b), bLabel: toolLabel(b) }).toBe(false);
  });
});
