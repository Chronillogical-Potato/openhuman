export type Locale =
  | 'en'
  | 'zh-CN'
  | 'hi'
  | 'es'
  | 'ar'
  | 'fr'
  | 'bn'
  | 'pt'
  | 'de'
  | 'ru'
  | 'id'
  | 'it'
  | 'ko'
  | 'pl'
  | 'tr';

export interface TranslationMap {
  [key: string]: string;
}
