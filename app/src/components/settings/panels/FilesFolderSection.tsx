import { useEffect, useRef, useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import { revealPath } from '../../../utils/openUrl';
import {
  type AgentPaths,
  openhumanGetAgentPaths,
  openhumanUpdateAgentPaths,
} from '../../../utils/tauriCommands';
import { Button } from '../../ui';
import { SettingsStatusLine, SettingsTextField } from '../controls';

/**
 * "Files folder" row in Settings → Agent OS access (#5505): where the files
 * the agent delivers are saved. Defaults to `~/OpenHuman/projects/Files`; the
 * core validates any other choice. A change applies to new files only.
 */
const FilesFolderSection = () => {
  const { t } = useT();
  const [paths, setPaths] = useState<AgentPaths | null>(null);
  const [input, setInput] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedNote, setSavedNote] = useState<string | null>(null);
  // Last write wins: a slow response must not overwrite a newer one.
  const seqRef = useRef(0);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const resp = await openhumanGetAgentPaths();
        if (cancelled) return;
        setPaths(resp.result);
        setInput(resp.result.files_dir);
      } catch {
        if (!cancelled) setError(t('settings.agentAccess.filesFolder.loadError'));
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const save = async (value: string) => {
    const seq = ++seqRef.current;
    setSaving(true);
    setError(null);
    setSavedNote(null);
    try {
      const resp = await openhumanUpdateAgentPaths({ files_dir: value });
      if (seq !== seqRef.current) return;
      setPaths(resp.result);
      setInput(resp.result.files_dir);
      setSavedNote(t('settings.agentAccess.filesFolder.saved'));
    } catch (e) {
      if (seq !== seqRef.current) return;
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      if (seq === seqRef.current) setSaving(false);
    }
  };

  const openFolder = async () => {
    if (!paths) return;
    setError(null);
    try {
      await revealPath(paths.files_dir);
    } catch {
      setError(t('settings.agentAccess.filesFolder.openError'));
    }
  };

  const trimmed = input.trim();
  const unchanged = paths !== null && trimmed === paths.files_dir;

  return (
    <div data-testid="files-folder-section">
      <div className="px-4 pt-3">
        <div className="text-sm font-medium text-content">
          {t('settings.agentAccess.filesFolder.label')}
        </div>
        <p className="mt-0.5 text-xs leading-relaxed text-content-muted">
          {t('settings.agentAccess.filesFolder.desc')}
        </p>
      </div>
      <div className="flex items-center gap-2 px-4 pb-2 pt-2">
        <SettingsTextField
          mono
          className="flex-1"
          value={input}
          onChange={e => setInput(e.target.value)}
          placeholder={paths?.default_files_dir}
          aria-label={t('settings.agentAccess.filesFolder.label')}
          disabled={paths === null}
          onKeyDown={e => {
            if (e.key === 'Enter' && trimmed && !unchanged) {
              e.preventDefault();
              void save(trimmed);
            }
          }}
          inputSize="sm"
          data-testid="files-folder-input"
        />
        <Button
          type="button"
          variant="secondary"
          size="sm"
          onClick={() => void save(trimmed)}
          disabled={saving || !trimmed || unchanged}
          analyticsId="settings-files-folder-save"
          data-testid="files-folder-save">
          {t('settings.agentAccess.filesFolder.save')}
        </Button>
        <Button
          type="button"
          variant="tertiary"
          size="sm"
          onClick={() => void openFolder()}
          disabled={paths === null}
          analyticsId="settings-files-folder-open"
          data-testid="files-folder-open">
          {t('settings.agentAccess.filesFolder.open')}
        </Button>
        {paths?.files_dir_source === 'override' && (
          <Button
            type="button"
            variant="tertiary"
            size="sm"
            onClick={() => void save('')}
            disabled={saving}
            analyticsId="settings-files-folder-reset"
            data-testid="files-folder-reset">
            {t('settings.agentAccess.filesFolder.reset')}
          </Button>
        )}
      </div>
      {(saving || savedNote || error) && (
        <div className="px-4 pb-3">
          <SettingsStatusLine
            saving={saving}
            savedNote={savedNote}
            error={error}
            savingLabel={t('settings.agentAccess.saving')}
          />
        </div>
      )}
    </div>
  );
};

export default FilesFolderSection;
