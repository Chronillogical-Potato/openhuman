import { describe, it } from 'vitest';

describe('sanity', () => {
  it('a pre-caught rejection does not fail the test', async () => {
    const p = Promise.reject(new Error('x'));
    p.catch(() => {});
    await new Promise(r => setTimeout(r, 10));
  });
});
