import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { fileURLToPath, URL } from 'node:url'
export default defineConfig({plugins:[react(),tailwindcss()],base:'/drivesync/',resolve:{alias:{'@':fileURLToPath(new URL('./src', import.meta.url))}},build:{rollupOptions:{input:{main:'index.html',development:'desenvolvimento/index.html',donations:'doacoes/index.html',privacy:'privacidade/index.html'}}}})
