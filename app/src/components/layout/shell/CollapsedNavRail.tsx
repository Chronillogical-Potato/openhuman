import { useMemo } from 'react';
import { useLocation, useNavigate } from 'react-router-dom';

import { NAV_TABS, type NavTab } from '../../../config/navConfig';
import { useT } from '../../../lib/i18n/I18nContext';
import { trackEvent } from '../../../services/analytics';
import { useAppSelector } from '../../../store/hooks';
import { selectUnreadCount } from '../../../store/notificationSlice';
import {
  SidebarMenu,
  SidebarMenuBadge,
  SidebarMenuButton,
  SidebarMenuItem,
  Tooltip,
} from '../../ui';
import { NavIcon } from './navIcons';
import { useCloudNavGate } from './useCloudNavGate';

/** Same active-route rules as the expanded {@link SidebarNav}. */
function matchActive(path: string, pathname: string): boolean {
  if (path === '/chat') return pathname.startsWith('/chat');
  if (path === '/settings') return pathname === '/settings' || pathname.startsWith('/settings/');
  if (path === '/flows') return pathname === '/flows' || pathname.startsWith('/flows/');
  if (path === '/home') return pathname === '/home';
  return pathname === path;
}

/**
 * Compact labelled rail footprint layered on `SidebarMenuButton`. The 72px
 * button fits the collapsed column with an icon over a one-line label; the
 * badge keeps the same positioning context.
 */
const RAIL_BTN = 'relative h-12 w-[72px] flex-col justify-center gap-1 rounded-lg px-1 py-1.5';
const RAIL_LABEL = 'max-w-full truncate text-[9px] font-medium leading-none';

/**
 * Compact labelled navigation shown in the collapsed root-shell rail: every
 * primary {@link NAV_TABS} destination plus Settings. Mirrors
 * {@link SidebarNav}'s routing and active-route rules.
 *
 * Renders outside the `Sidebar` column (the column is unmounted while
 * collapsed), which is fine: the menu primitives read no sidebar context.
 */
export default function CollapsedNavRail() {
  const { t } = useT();
  const location = useLocation();
  const navigate = useNavigate();
  const unreadCount = useAppSelector(state => selectUnreadCount(state.notifications.items));

  const cloudAllowed = useCloudNavGate();
  const tabs = useMemo(
    () =>
      NAV_TABS.filter(tab => !tab.cloudOnly || cloudAllowed).map(tab => ({
        ...tab,
        label: t(tab.labelKey),
      })),
    [cloudAllowed, t]
  );
  const activeTab = tabs.find(tab => matchActive(tab.path, location.pathname));

  const handleClick = (tab: NavTab, active: boolean) => {
    if (!active) {
      trackEvent('tab_bar_change', {
        from_tab: activeTab?.id ?? 'unknown',
        to_tab: tab.id,
        from_path: location.pathname,
        to_path: tab.path,
      });
    }
    navigate(tab.path);
  };

  const settingsActive = matchActive('/settings', location.pathname);

  return (
    <nav aria-label={t('nav.home')}>
      <SidebarMenu className="items-center gap-2">
        {/* Primary nav destinations */}
        {tabs.map(tab => {
          const active = matchActive(tab.path, location.pathname);
          const showBadge = tab.id === 'notifications' && unreadCount > 0;
          return (
            <SidebarMenuItem key={tab.id}>
              <Tooltip label={tab.label}>
                <SidebarMenuButton
                  isActive={active}
                  data-walkthrough={tab.walkthroughAttr}
                  onClick={() => handleClick(tab, active)}
                  aria-label={tab.label}
                  className={RAIL_BTN}>
                  <NavIcon id={tab.id} className="h-4 w-4" />
                  <span className={RAIL_LABEL}>{tab.label}</span>
                  {showBadge && (
                    <SidebarMenuBadge tone="attention" className="absolute right-1 top-1 ml-0">
                      {unreadCount > 9 ? '9+' : unreadCount}
                    </SidebarMenuBadge>
                  )}
                </SidebarMenuButton>
              </Tooltip>
            </SidebarMenuItem>
          );
        })}

        {/* Settings — reached via the header gear when expanded, which is hidden
            in the collapsed rail, so it gets its own icon here. */}
        <SidebarMenuItem>
          <Tooltip label={t('nav.settings')}>
            <SidebarMenuButton
              isActive={settingsActive}
              onClick={() => navigate('/settings')}
              aria-label={t('nav.settings')}
              data-analytics-id="collapsed-rail-settings"
              className={RAIL_BTN}>
              <NavIcon id="settings" className="h-4 w-4" />
              <span className={RAIL_LABEL}>{t('nav.settings')}</span>
            </SidebarMenuButton>
          </Tooltip>
        </SidebarMenuItem>
      </SidebarMenu>
    </nav>
  );
}
