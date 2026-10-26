import { Languages } from 'lucide-react'
import { useI18n } from '@/i18n/useI18n'
import type { Locale } from '@/i18n/translations'
export function LanguageSwitcher() {
  const { locale, setLocale, t } = useI18n()
  return <label className="language-switcher"><Languages size={16} aria-hidden="true" /><span className="sr-only">{t('Idioma')}</span><select aria-label={t('Idioma')} value={locale} onChange={event => setLocale(event.target.value as Locale)}><option value="pt-BR" lang="pt-BR">Português</option><option value="en" lang="en">English</option></select></label>
}
