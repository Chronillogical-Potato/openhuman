/**
 * Regression test for #3785 — "UI elements hidden when window is too small,
 * obscuring actionable error states".
 *
 * On the Human page the chat embed renders the sidebar variant of
 * Conversations. Its composer footer stacks the upsell/error banners, the
 * actionable error CTAs (e.g. the voice-transcription "Setup" link), and the
 * composer itself in a single block inside the `overflow-hidden` mainPanel.
 * The footer used to be a plain `shrink-0` block, so on a short window its
 * natural height exceeded the panel and its bottom was clipped with no scroll
 * affordance — the composer and the fix button became unreachable.
 *
 * The fix lets the footer SHRINK and scroll instead of staying rigid: dropping
 * `shrink-0` and adding `min-h-0 overflow-y-auto` makes the flex algorithm
 * cap it to the available height and scroll it internally. jsdom does not lay
 * out, so we assert the footer is class-wise scroll-capable + shrinkable, which
 * is what prevents the silent clipping from coming back.
 */
import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { act, render } from '@testing-library/react';
import { Provider } from 'react-redux';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { SidebarSlotOutlet, SidebarSlotProvider } from '../../components/layout/shell/SidebarSlot';
import chatRuntimeReducer from '../../store/chatRuntimeSlice';
import layoutReducer from '../../store/layoutSlice';
import socketReducer from '../../store/socketSlice';
import themeReducer from '../../store/themeSlice';
import threadReducer from '../../store/threadSlice';
import type { Thread } from '../../types/thread';

// ── Hoisted mock state ─────────────────────────────────────────────────────

const { mockGetThreads, mockGetThreadMessages, mockUseUsageState } = vi.hoisted(() => ({
  mockGetThreads: vi.fn().mockResolvedValue({ threads: [], count: 0 }),
  mockGetThreadMessages: vi.fn().mockResolvedValue({ messages: [], count: 0 }),
  mockUseUsageState: vi.fn(() => ({
    teamUsage: null as null | {
      cycleBudgetUsd: number;
      remainingUsd: number;
      cycleSpentUsd: number;
      cycleEndsAt: string | null;
    },
    currentPlan: null,
    currentTier: 'FREE' as 'FREE' | 'BASIC' | 'PRO',
    isFreeTier: true,
    usagePct: 0,
    isNearLimit: false,
    isAtLimit: false,
    isBudgetExhausted: false,
    shouldShowBudgetCompletedMessage: false,
    isLoading: false,
    refresh: vi.fn(),
  })),
}));

// ── Module mocks (mirror Conversations.render.test.tsx's known-good set) ────

vi.mock('../../services/chatService', () => ({
  chatCancel: vi.fn(),
  chatSend: vi.fn().mockResolvedValue(undefined),
  subscribeChatEvents: vi.fn(() => () => {}),
  useRustChat: vi.fn(() => true),
}));

vi.mock('../../services/api/threadApi', () => ({
  threadApi: {
    createNewThread: vi.fn().mockResolvedValue({ id: 'new-thread', labels: [] }),
    getThreads: mockGetThreads,
    getThreadMessages: mockGetThreadMessages,
    getTurnState: vi.fn().mockResolvedValue(null),
    appendMessage: vi.fn().mockResolvedValue({}),
    deleteThread: vi.fn().mockResolvedValue({ deleted: true }),
    generateTitleIfNeeded: vi.fn().mockResolvedValue({}),
    updateMessage: vi.fn().mockResolvedValue({}),
    purge: vi.fn().mockResolvedValue({}),
    updateLabels: vi.fn().mockResolvedValue({}),
    updateTitle: vi.fn().mockResolvedValue({}),
    persistReaction: vi.fn().mockResolvedValue({}),
  },
}));
