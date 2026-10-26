import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import ts from 'typescript'

async function loadModule(path) {
  const source = readFileSync(new URL(path, import.meta.url), 'utf8')
  const { outputText } = ts.transpileModule(source, { compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 } })
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`)
}
const { resolveLocale, detectLocale } = await loadModule('../src/i18n/locale.ts')
const { translations } = await loadModule('../src/i18n/translations.ts')

test('an explicit selection wins over the browser language', () => {
  assert.equal(resolveLocale('en', 'pt-BR'), 'en')
  assert.equal(resolveLocale('pt-BR', 'en-US'), 'pt-BR')
})
test('Portuguese variants select pt-BR; other languages default to English', () => {
  for (const language of ['pt', 'pt-BR', 'pt-PT', 'PT-br']) assert.equal(resolveLocale(null, language), 'pt-BR')
  for (const language of ['en-US', 'fr-FR', 'de', '']) assert.equal(resolveLocale(null, language), 'en')
  assert.equal(resolveLocale('invalid', 'pt-PT'), 'pt-BR')
})
test('automatic detection still works when storage is blocked', () => {
  globalThis.window = { localStorage: { getItem() { throw new Error('denied') } }, navigator: { language: 'pt-PT' } }
  try { assert.equal(detectLocale(), 'pt-BR') } finally { delete globalThis.window }
})
test('both dictionaries cover every static message and gallery caption', () => {
  assert.deepEqual(Object.keys(translations.en), Object.keys(translations['pt-BR']))
  const keys = new Set()
  const files = ['../src/App.tsx', '../src/components/screenshot.tsx', '../src/components/screenshot-gallery.tsx', '../src/i18n/I18nProvider.tsx']
  for (const file of files) {
    const source = ts.createSourceFile(file, readFileSync(new URL(file, import.meta.url), 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX)
    function visit(node) {
      if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
        if (node.expression.text === 't' && ts.isStringLiteral(node.arguments[0])) keys.add(node.arguments[0].text)
        if (['full', 'detail'].includes(node.expression.text)) node.arguments.slice(1).forEach(value => { if (ts.isStringLiteral(value)) keys.add(value.text) })
      }
      if (ts.isPropertyAssignment(node) && ['label', 'title', 'description'].includes(node.name.getText(source)) && ts.isStringLiteral(node.initializer)) keys.add(node.initializer.text)
      if (ts.isArrayLiteralExpression(node) && node.elements.length === 2 && node.elements.every(ts.isStringLiteral)) node.elements.forEach(value => keys.add(value.text))
      ts.forEachChild(node, visit)
    }
    visit(source)
  }
  for (const key of keys) {
    assert.ok(translations.en[key], `Missing English translation: ${key}`)
    assert.ok(translations['pt-BR'][key], `Missing Portuguese translation: ${key}`)
  }
  for (const [key, english] of Object.entries(translations.en)) assert.deepEqual(english.match(/\{\w+\}/g) ?? [], key.match(/\{\w+\}/g) ?? [], key)
})
