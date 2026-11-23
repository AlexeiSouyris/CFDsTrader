import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// El gateway del motor (#231) escucha donde diga `config/gateway.toml`; en desarrollo, `/api` va
// ahí. Así la dirección vive en un solo lugar.
const gatewayToml = readFileSync(new URL('../config/gateway.toml', import.meta.url), 'utf8')
const gatewayBind = /^bind\s*=\s*"([^"]+)"/m.exec(gatewayToml)?.[1]
if (!gatewayBind) throw new Error('config/gateway.toml: falta [server] bind')

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    // `@/` apunta a `src/`: es el alias que usan los componentes de 21st.dev / shadcn.
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  server: {
    proxy: { '/api': `http://${gatewayBind}` },
    // El diccionario interactivo lee `docs/GLOSARIO.md` (única fuente de las definiciones, #171).
    fs: {
      allow: [
        fileURLToPath(new URL('.', import.meta.url)),
        fileURLToPath(new URL('../docs', import.meta.url)),
      ],
    },
  },
})
