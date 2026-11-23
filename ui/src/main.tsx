import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

// PWA (#307): el service worker guarda solo el armazón, nunca /api (ver `public/sw.js`). Solo en
// el build de producción: en desarrollo, la caché taparía los cambios de Vite.
if (import.meta.env.PROD && 'serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/sw.js').catch((e: unknown) => {
      console.warn('no se pudo registrar el service worker', e)
    })
  })
}
