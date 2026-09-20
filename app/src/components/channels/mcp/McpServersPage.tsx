/**
 * Connections → MCP Servers: the user's tool servers, said three ways.
 *
 * The page owns its own header — title, description and the tab strip — the
 * way the LLM page does, so the tabs sit in the header's chrome rather than in
 * a card under it. **Servers** is the list, **mcp.json** the same configuration
 * as one document, **Registry** the browse-only directories. The first two are
 * tabs and not two pages because they are not two things: both go through the
 * same core RPCs into the same store.
 */
import { useState } from 'react';

import { useT } from '../../../lib/i18n/I18nContext';
import SettingsTabbedPage from '../../settings/layout/SettingsTabbedPage';
import BetaIndicator from '../../ui/BetaIndicator';
import McpServersTab, { type McpPageTab } from './McpServersTab';

interface McpServersPageProps {
  initialTab?: McpPageTab;
}

const McpServersPage = ({ initialTab = 'servers' }: McpServersPageProps) => {
  const { t } = useT();
  const [tab, setTab] = useState<McpPageTab>(initialTab);

  return (
    <SettingsTabbedPage
      title={t('connections.tabs.mcp')}
      description={t('connections.header.mcp')}
      headerAction={<BetaIndicator />}
      tabs={[
        { id: 'servers', label: t('mcp.tab.section.servers') },
        { id: 'json', label: t('mcp.tab.section.json') },
        { id: 'registry', label: t('mcp.tab.section.registry') },
      ]}
      value={tab}
      onChange={setTab}
      tabsAriaLabel={t('mcp.tab.tablistAria')}
      tabsTestIdPrefix="mcp-page-tab">
      <McpServersTab tab={tab} onTabChange={setTab} />
    </SettingsTabbedPage>
  );
};

export default McpServersPage;
