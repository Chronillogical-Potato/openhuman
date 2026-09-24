import { describe, expect, it } from 'vitest';

import en from '../../../lib/i18n/en';
import { phraseKey, TOOL_PHRASES, type ToolPhraseId } from './toolPhrases';

const enMap = en as Record<string, string>;
const placeholders = (value: string) => [...value.matchAll(/\{(\w+)\}/g)].map(m => m[1]).sort();

describe('tool phrases', () => {
  it.each(Object.keys(TOOL_PHRASES) as ToolPhraseId[])(
    '%s is served by en.ts with the same English',
    id => {
      for (const tense of ['active', 'done'] as const) {
        expect(enMap[phraseKey(id, tense)]).toBe(TOOL_PHRASES[id][tense]);
      }
    }
  );

  it.each(Object.keys(TOOL_PHRASES) as ToolPhraseId[])(
    '%s reads differently once done, with the same placeholders',
    id => {
      const { active, done } = TOOL_PHRASES[id];
      expect(active).not.toBe(done);
      expect(placeholders(active)).toEqual(placeholders(done));
    }
  );
});
