import { describe, expect, it } from 'vitest';

import type { LocalProviderSnapshot } from '../../../../services/api/aiSettingsApi';
import { deriveOllamaState } from './useAISettingsState';

function makeSnapshot(
  overrides: Partial<{
    state: string;
    ollama_running: boolean;
    ollama_status: 'running' | 'degraded' | 'stopped' | undefined;
  }>
): LocalProviderSnapshot {
  return {
    status: overrides.state != null ? ({ state: overrides.state } as never) : null,
    diagnostics:
      overrides.ollama_running != null || overrides.ollama_status != null
        ? ({
            ollama_running: overrides.ollama_running ?? false,
            ollama_status: overrides.ollama_status,
            ollama_base_url: 'http://localhost:11434',
            ollama_binary_path: null,
            installed_models: [],
            expected: {
              chat_model: '',
              chat_found: false,
              embedding_model: '',
              embedding_found: false,
              vision_model: '',
              vision_found: false,
            },
            issues: [],
            repair_actions: [],
            ok: false,
          } as never)
        : null,
    installedModels: [],
  };
}

describe('deriveOllamaState', () => {
  it('returns stopped for null snapshot', () => {
    expect(deriveOllamaState(null)).toBe('stopped');
  });

  it('returns disabled when status.state is disabled', () => {
    expect(deriveOllamaState(makeSnapshot({ state: 'disabled' }))).toBe('disabled');
  });

  it('returns degraded when ollama_status is degraded (takes priority over ollama_running)', () => {
    expect(
      deriveOllamaState(makeSnapshot({ ollama_running: true, ollama_status: 'degraded' }))
    ).toBe('degraded');
  });

  it('returns running when ollama_running is true and status is not degraded', () => {
    expect(
      deriveOllamaState(makeSnapshot({ ollama_running: true, ollama_status: 'running' }))
    ).toBe('running');
    // ollama_status absent falls through to ollama_running check.
    expect(deriveOllamaState(makeSnapshot({ ollama_running: true }))).toBe('running');
  });

  it('returns unreachable when the core cannot reach the user-run endpoint', () => {
    expect(deriveOllamaState(makeSnapshot({ state: 'unreachable', ollama_running: false }))).toBe(
      'unreachable'
    );
  });

  it('a live diagnostics probe wins over a stale unreachable status', () => {
    expect(deriveOllamaState(makeSnapshot({ state: 'unreachable', ollama_running: true }))).toBe(
      'running'
    );
  });

  it('returns degraded when the core reports degraded without a diagnostics verdict', () => {
    expect(deriveOllamaState(makeSnapshot({ state: 'degraded' }))).toBe('degraded');
  });

  it('maps retired managed-runtime states to stopped', () => {
    // The app no longer installs or downloads anything; an older core that
    // still reports these must not surface a phantom progress state.
    for (const state of ['downloading', 'installing', 'starting', 'missing']) {
      expect(deriveOllamaState(makeSnapshot({ state }))).toBe('stopped');
    }
  });

  it('returns stopped for an idle or ready status without a running probe', () => {
    expect(deriveOllamaState(makeSnapshot({ state: 'idle' }))).toBe('stopped');
    expect(deriveOllamaState(makeSnapshot({ state: 'ready' }))).toBe('stopped');
  });

  it('returns stopped as catch-all', () => {
    expect(deriveOllamaState(makeSnapshot({}))).toBe('stopped');
  });
});
