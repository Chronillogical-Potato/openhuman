// Exercise the security settings, whole-tool grant list, policy diagnostics,
// and approval history against the throwaway core and mock backend.
export default async function securityApprovalUi({ page, rpc, urls, log, screenshot }) {
  await page.goto(`${urls.app}/#/settings/security`);
  await page.getByText('Always-allowed tools', { exact: true }).waitFor();
  await page.getByText('No tools are always allowed.', { exact: true }).waitFor();

  const rawGrants = await rpc('openhuman.approval_list_tool_trust');
  const grants = Array.isArray(rawGrants) ? rawGrants : rawGrants.result;
  if (grants.length !== 0) {
    throw new Error(`expected a fresh workspace to have no tool grants, got ${grants.length}`);
  }
  await screenshot('security-tool-grants');

  await page.goto(`${urls.app}/#/settings/tool-policy-diagnostics`);
  await page.getByText('Active security policy', { exact: true }).waitFor();
  await screenshot('active-security-policy');

  await page.goto(`${urls.app}/#/settings/approval-history`);
  await page.getByTestId('approval-history-panel').waitFor();
  await page
    .locator('[data-testid="approval-history-empty"], [data-testid="approval-history-list"]')
    .first()
    .waitFor();
  await screenshot('approval-history');
  log('security grants, policy diagnostics, and approval history rendered');
}
