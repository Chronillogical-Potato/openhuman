import { createSlice, type PayloadAction } from '@reduxjs/toolkit';

import { resetUserScopedState } from './resetActions';

export interface WalletPreferencesState {
  hiddenTokenKeys: string[];
}

const initialState: WalletPreferencesState = { hiddenTokenKeys: [] };

const walletPreferencesSlice = createSlice({
  name: 'walletPreferences',
  initialState,
  reducers: {
    toggleTokenHidden(state, action: PayloadAction<{ tokenKey: string }>) {
      const { tokenKey } = action.payload;
      const index = state.hiddenTokenKeys.indexOf(tokenKey);
      if (index === -1) {
        state.hiddenTokenKeys.push(tokenKey);
      } else {
        state.hiddenTokenKeys.splice(index, 1);
      }
    },
  },
  extraReducers: builder => {
    builder.addCase(resetUserScopedState, () => initialState);
  },
});

export const { toggleTokenHidden } = walletPreferencesSlice.actions;

export default walletPreferencesSlice.reducer;
