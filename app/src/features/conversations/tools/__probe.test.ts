import { describe, expect, it } from 'vitest';

import { describeToolCall, toolLabel } from './toolPresentation';

describe('probe', () => {
  it('prints labels', () => {
    const a = describeToolCall({
      name: 'stock_quote',
      args: JSON.stringify({ symbol: 'AAPL' }),
      status: 'success',
      serverLabel: 'Stock quote',
      serverDetail: 'AAPL',
    });
    expect({ a: JSON.stringify(a), aLabel: toolLabel(a) }).toBe(false);
  });
});
