import { describe, expect, it } from 'vitest';

import { describeToolCall, toolLabel } from './toolPresentation';

describe('probe', () => {
  it('prints labels', () => {
    const a = describeToolCall({
      name: 'acme_widget_ping',
      args: JSON.stringify({ symbol: 'AAPL' }),
      status: 'success',
      serverLabel: 'Widget ping',
      serverDetail: 'AAPL',
    });
    expect({ a: JSON.stringify(a), aLabel: toolLabel(a) }).toBe(false);
  });
});
