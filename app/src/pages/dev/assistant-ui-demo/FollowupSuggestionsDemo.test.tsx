import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { MOCK_CHAT_SUGGESTIONS_EVENT } from './assistantUiMock/mockScript';
import { FollowupSuggestionsDemo } from './FollowupSuggestionsDemo';

describe('FollowupSuggestionsDemo', () => {
  it('renders one follow-up chip per suggestion in the fixture chat_suggestions event', () => {
    render(<FollowupSuggestionsDemo />);

    const chips = screen.getAllByRole('button');
    expect(chips).toHaveLength(MOCK_CHAT_SUGGESTIONS_EVENT.suggestions.length);
    // Labelled suggestions show their label; the unlabelled one shows its prompt.
    for (const { prompt, label } of MOCK_CHAT_SUGGESTIONS_EVENT.suggestions) {
      expect(screen.getByRole('button', { name: label ?? prompt })).toBeTruthy();
    }
  });
});
