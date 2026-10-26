import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App'
import { I18nProvider } from './i18n/I18nProvider'
import '@fontsource/cantarell/latin-400.css'
import '@fontsource/cantarell/latin-700.css'
import './index.css'
ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><I18nProvider><App /></I18nProvider></React.StrictMode>)
