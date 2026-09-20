/**
 * Connections → Skills: the user's skills, said two ways.
 *
 * The page owns its own header — title, description and the tab strip — the
 * way the MCP page does. **Installed** first: what is on this machine, with
 * run, edit and remove on every row. **Registry** last: the catalogues to
 * install from.
 */
import { useState } from 'react';

import { useT } from '../../lib/i18n/I18nContext';
import SettingsTabbedPage from '../settings/layout/SettingsTabbedPage';
import BetaIndicator from '../ui/BetaIndicator';
import SkillsExplorerTab, { type ExplorerView } from './SkillsExplorerTab';

interface SkillsPageProps {
  onToast?: (toast: { type: 'success' | 'error'; title: string; message?: string }) => void;
  initialTab?: ExplorerView;
}

const SkillsPage = ({ onToast, initialTab = 'installed' }: SkillsPageProps) => {
  const { t } = useT();
  const [tab, setTab] = useState<ExplorerView>(initialTab);

  return (
    <SettingsTabbedPage
      title={t('connections.tabs.skills')}
      description={t('connections.header.skills')}
      headerAction={<BetaIndicator />}
      tabs={[
        { id: 'installed', label: t('skills.explorer.installedTab') },
        { id: 'registry', label: t('skills.explorer.registryTab') },
      ]}
      value={tab}
      onChange={setTab}
      tabsAriaLabel={t('skills.explorer.title')}
      tabsTestIdPrefix="skill-explorer-tab">
      <SkillsExplorerTab view={tab} onToast={onToast} />
    </SettingsTabbedPage>
  );
};

export default SkillsPage;
