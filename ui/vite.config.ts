import { fileURLToPath, URL } from 'node:url'

import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    // `@/` apunta a `src/`: es el alias que usan los componentes de 21st.dev / shadcn.
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
})
