import { useCallback, useEffect, useMemo, useState, type ReactNode } from 'react'
import { I18nContext, type Translate } from './context'
import { detectLocale, STORAGE_KEY } from './locale'
import { translations, type Locale, type TranslationKey } from './translations'

const metadata = {
  app: ['DriveSync — Google Drive para Linux', 'Um cliente independente do Google Drive para Linux. Conheça o aplicativo, acompanhe o desenvolvimento e participe.'],
  development: ['Desenvolvimento — DriveSync', 'Acompanhe o estado do DriveSync, suas prioridades e o processo de desenvolvimento.'],
  donations: ['Doações — DriveSync', 'Conheça as formas de apoiar o desenvolvimento e a manutenção do DriveSync.'],
  privacy: ['Política de privacidade — DriveSync', 'Saiba como o site e o aplicativo DriveSync utilizam dados e permissões do Google.'],
} as const

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(detectLocale)
  const t = useCallback<Translate>((key, variables) => {
    const message = translations[locale][key as TranslationKey] ?? key
    return variables ? Object.entries(variables).reduce((text, [name, value]) => text.replaceAll(`{${name}}`, String(value)), message) : message
  }, [locale])
  const setLocale = useCallback((next: Locale) => {
    setLocaleState(next)
    try { window.localStorage.setItem(STORAGE_KEY, next) } catch { /* The in-memory preference still applies without storage. */ }
  }, [])
  useEffect(() => {
    const path = window.location.pathname.slice(import.meta.env.BASE_URL.length)
    const key = path.startsWith('desenvolvimento/') ? 'development' : path.startsWith('doacoes/') ? 'donations' : path.startsWith('privacidade/') ? 'privacy' : 'app'
    const [title, description] = metadata[key]
    document.documentElement.lang = locale
    document.title = t(title)
    for (const [selector, content] of [
      ['meta[name="description"]', t(description)],
      ['meta[property="og:title"]', t(title)],
      ['meta[property="og:description"]', t(description)],
      ['meta[property="og:locale"]', locale === 'pt-BR' ? 'pt_BR' : 'en_US'],
    ]) {
      document.querySelector<HTMLMetaElement>(selector)?.setAttribute('content', content)
    }
  }, [locale, t])
  const value = useMemo(() => ({ locale, numberLocale: locale, setLocale, t }), [locale, setLocale, t])
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>
}
