import { render, renderHook, screen } from '@testing-library/react';
import { SlashIcon } from 'lucide-react';
import { describe, expect, it } from 'vitest';

import {
  applyMention,
  type ComposerCommand,
  ComposerCommandItem,
  ComposerMenu,
  type ComposerPerson,
  useMentionMatches,
  useSlashMatches,
} from './composer';

const COMMANDS: ComposerCommand[] = [
  { name: 'plan', description: 'Plan first', icon: SlashIcon },
  { name: 'build', description: 'Build it', icon: SlashIcon },
  { name: 'publish', description: 'Ship it', icon: SlashIcon },
];

const PEOPLE: ComposerPerson[] = [
  { name: 'Researcher', role: 'agent' },
  { name: 'Riley', role: 'human' },
  { name: 'Coder', role: 'agent' },
];

describe('useSlashMatches', () => {
  it('returns commands whose name starts with the slash query', () => {
    const { result } = renderHook(() => useSlashMatches('/p', COMMANDS));
    expect(result.current.map(c => c.name)).toEqual(['plan', 'publish']);
  });

  it('returns nothing when the value is not a slash command', () => {
    const { result } = renderHook(() => useSlashMatches('plan', COMMANDS));
    expect(result.current).toEqual([]);
  });
});

describe('useMentionMatches', () => {
  it('matches people against a trailing @mention, case-insensitively', () => {
    const { result } = renderHook(() => useMentionMatches('ask @r', PEOPLE));
    expect(result.current.map(p => p.name)).toEqual(['Researcher', 'Riley']);
  });

  it('returns nothing when the caret is not in a mention', () => {
    const { result } = renderHook(() => useMentionMatches('ask @r now', PEOPLE));
    expect(result.current).toEqual([]);
  });
});

describe('applyMention', () => {
  it('replaces the trailing @mention with the chosen name', () => {
    expect(applyMention('ask @re', 'Researcher')).toBe('ask @Researcher ');
  });
});

describe('ComposerCommandItem', () => {
  it('renders the command inside an open menu', () => {
    render(
      <ComposerMenu open>
        <ComposerCommandItem command={COMMANDS[0]!} active />
      </ComposerMenu>
    );
    expect(screen.getByText('/plan')).toBeInTheDocument();
    expect(screen.getByText('Plan first')).toBeInTheDocument();
  });
});
