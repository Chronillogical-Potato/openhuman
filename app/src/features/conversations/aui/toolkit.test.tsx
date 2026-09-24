import { describe, expect, it } from 'vitest';

import { SubagentTaskCard } from './SubagentTaskCard';
import { buildOpenHumanToolkit, openHumanToolEntries } from './toolkit';

describe('buildOpenHumanToolkit', () => {
  it('registers the task tool against the shared delegation card', () => {
    const toolkit = buildOpenHumanToolkit();
    expect(toolkit.task).toBeDefined();
    expect(toolkit.task.type).toBe('backend');
    expect(toolkit.task.render).toBe(SubagentTaskCard);
  });

  it('never declares description/parameters on a backend entry', () => {
    // `type: 'backend'` requires these to stay `undefined`: they are the
    // core's tool schema, sent to the model over the wire, not something the
    // frontend toolkit re-declares.
    for (const entry of Object.values(openHumanToolEntries())) {
      expect(entry.type).toBe('backend');
    }
  });
});
