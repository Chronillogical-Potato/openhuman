/**
 * Retired onboarding step — kept on disk, not routed.
 *
 * The custom wizard was cut to inference → search → vault; voice, OAuth and
 * embeddings are all still configurable from Settings, so the step was the
 * only thing removed. The file stays so reviving the step is a route entry
 * rather than a rewrite. Note `pnpm knip` lists this as an unused file;
 * knip is not a CI gate, and deleting it is a reviewer's call.
 */
import ComposioPanel from '../../../components/settings/panels/ComposioPanel';
import CustomWizardConfigPage from './CustomWizardConfigPage';

const CustomOAuthPage = () => (
  <CustomWizardConfigPage
    stepKey="oauth"
    configureContent={<ComposioPanel embedded managedAuthEnabled={false} />}
  />
);

export default CustomOAuthPage;
