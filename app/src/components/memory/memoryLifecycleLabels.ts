/**
 * Labels for the memory lifecycle surface: brain source types. Literal `t()` keys (not built from the value) so the
 * i18n scanner can see every one of them; an unknown value shows as-is.
 */
type Translate = (key: string, fallback?: string) => string;

/** A brain source type's label: the well-known ones translated, any other id verbatim. */
export function brainSourceLabel(source: string, t: Translate): string {
  switch (source) {
    case 'files':
      return t('memoryPage.brain.source.files');
    case 'pdf':
      return t('memoryPage.brain.source.pdf');
    case 'markdown':
      return t('memoryPage.brain.source.markdown');
    default:
      return source;
  }
}
