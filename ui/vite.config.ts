import { readFileSync } from 'node:fs'
import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig, loadEnv } from 'vite'

import { gatewayTokenFor, TOKEN_HEADER } from './src/lib/gateway-token.ts'

// El gateway del motor (#231) escucha donde diga `config/gateway.toml`; en desarrollo, `/api` va
// ahí. Así la dirección vive en un solo lugar.
const gatewayToml = readFileSync(new URL('../config/gateway.toml', import.meta.url), 'utf8')
const gatewayBind = /^bind\s*=\s*"([^"]+)"/m.exec(gatewayToml)?.[1]
if (!gatewayBind) throw new Error('config/gateway.toml: falta [server] bind')

// Token anti-CSRF de los POST del broker simulado (#292): sale de `CFDS_GATEWAY_TOKEN` (del entorno
// o del `.env` de la raíz, que no se commitea) y lo agrega el proxy, del lado del servidor. Nunca
// llega al bundle del cliente: Vite solo expone al cliente las variables `VITE_`.
const gatewayToken =
  process.env.CFDS_GATEWAY_TOKEN ||
  loadEnv('', fileURLToPath(new URL('..', import.meta.url)), 'CFDS_GATEWAY_TOKEN')
    .CFDS_GATEWAY_TOKEN ||
  undefined

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    // `@/` apunta a `src/`: es el alias que usan los componentes de 21st.dev / shadcn.
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  server: {
    proxy: {
      '/api': {
        target: `http://${gatewayBind}`,
        configure: (proxy) =>
          proxy.on('proxyReq', (proxyReq, req) => {
            // nunca se reenvía un token que mande el navegador: solo el del servidor
            proxyReq.removeHeader(TOKEN_HEADER)
            const token = gatewayTokenFor(
              {
                method: req.method,
                url: req.url,
                origin: req.headers.origin,
                host: req.headers.host,
              },
              gatewayToken,
            )
            if (token) proxyReq.setHeader(TOKEN_HEADER, token)
          }),
      },
    },
    // El diccionario interactivo lee `docs/GLOSARIO.md` (única fuente de las definiciones, #171).
    fs: {
      allow: [
        fileURLToPath(new URL('.', import.meta.url)),
        fileURLToPath(new URL('../docs', import.meta.url)),
      ],
    },
  },
})
