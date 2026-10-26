import type { Locale } from './translations'
export const STORAGE_KEY = 'drivesync.website.locale'
export function resolveLocale(savedLocale: string | null, browserLanguage: string): Locale {
  if (savedLocale === 'en' || savedLocale === 'pt-BR') return savedLocale
  return browserLanguage.toLowerCase().startsWith('pt') ? 'pt-BR' : 'en'
}
export function detectLocale(): Locale {
  let saved: string | null = null
  try { saved = window.localStorage.getItem(STORAGE_KEY) } catch { /* Storage can be unavailable in restricted contexts. */ }
  return resolveLocale(saved, window.navigator.language)
}
