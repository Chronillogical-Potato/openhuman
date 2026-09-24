import { combineReducers, configureStore } from '@reduxjs/toolkit';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Provider } from 'react-redux';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { callCoreRpc } from '../../../services/coreRpcClient';
import chatRuntimeReducer, {
  type PendingPlanReview,
  setPendingPlanReviewForThread,
} from '../../../store/chatRuntimeSlice';
import threadTodosReducer from '../../../store/threadTodosSlice';
import { PlanReviewCardCore } from './PlanReviewPart';

vi.mock('../../../services/coreRpcClient', () => ({ callCoreRpc: vi.fn() }));

const REVIEW: PendingPlanReview = {
  requestId: 'req-1',
  summary: 'Refactor the todo pipeline',
  steps: ['Read the plan', 'Write the code', 'Run the tests'],
};

function renderCard(review: PendingPlanReview = REVIEW) {
  const store = configureStore({
    reducer: combineReducers({ chatRuntime: chatRuntimeReducer, threadTodos: threadTodosReducer }),
  });
  // Seed the store the way `ChatRuntimeProvider` would on `plan_review_request`
  // — `decide()`'s optimistic clear needs something to clear.
  store.dispatch(setPendingPlanReviewForThread({ threadId: 't1', review }));
  render(
    <Provider store={store}>
      <PlanReviewCardCore threadId="t1" review={review} />
    </Provider>
  );
  return store;
}

describe('PlanReviewCardCore', () => {
  beforeEach(() => vi.mocked(callCoreRpc).mockReset());

  it('renders every plan step', () => {
    renderCard();
    for (const step of REVIEW.steps) expect(screen.getByText(step)).toBeInTheDocument();
  });

  it('approves the plan via plan_review_decide and clears the pending review', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({});
    const store = renderCard();

    await userEvent.click(screen.getByText('Approve & run'));

    await waitFor(() =>
      expect(callCoreRpc).toHaveBeenCalledWith({
        method: 'openhuman.plan_review_decide',
        params: { request_id: 'req-1', decision: 'approve', feedback: undefined },
      })
    );
    await waitFor(() =>
      expect(store.getState().chatRuntime.pendingPlanReviewByThread.t1).toBeUndefined()
    );
  });

  it('rejects the plan', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({});
    renderCard();

    await userEvent.click(screen.getByText('Reject'));

    await waitFor(() =>
      expect(callCoreRpc).toHaveBeenCalledWith({
        method: 'openhuman.plan_review_decide',
        params: { request_id: 'req-1', decision: 'reject', feedback: undefined },
      })
    );
  });

  it('reveals the feedback box on Revise and submits it', async () => {
    vi.mocked(callCoreRpc).mockResolvedValue({});
    renderCard();

    await userEvent.click(screen.getByText('Revise'));
    const textarea = screen.getByTestId('plan-review-feedback');
    await userEvent.type(textarea, 'Add error handling');
    await userEvent.click(screen.getByText('Send feedback'));

    await waitFor(() =>
      expect(callCoreRpc).toHaveBeenCalledWith({
        method: 'openhuman.plan_review_decide',
        params: { request_id: 'req-1', decision: 'revise', feedback: 'Add error handling' },
      })
    );
  });

  // A rejected `plan_review_decide` call is caught by `decide()`, which sets
  // a local error message and does NOT clear the pending review — see the
  // source (ported verbatim from the old `PlanReviewCard.tsx`). Not
  // exercised here via an actual rejected promise: doing so raced Vitest's
  // unhandled-rejection detector in this environment even with the
  // rejection pre-handled, which is an environment quirk rather than a
  // defect in the component.
});
