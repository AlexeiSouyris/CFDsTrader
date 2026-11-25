/**
 * Cuándo aparece la intro "El mercado espera" (#581, etapa 9): solo la primera vez en este
 * navegador, antes del ingreso. Se recuerda en `localStorage`; si el navegador no deja leerlo,
 * la intro no aparece (para no repetirla en cada carga).
 *
 * En los tests no aparece (salvo que un test la pida con `VITE_INTRO_PREVIEW=1`); en desarrollo,
 * `VITE_INTRO_PREVIEW=1` la fuerza aunque ya se haya visto.
 */
export const INTRO_KEY = 'cfds.intro.seen'

export const introPreview = () => import.meta.env.DEV && import.meta.env.VITE_INTRO_PREVIEW === '1'

export function introSeen(): boolean {
  try {
    return localStorage.getItem(INTRO_KEY) === '1'
  } catch {
    return true
  }
}

export function markIntroSeen() {
  try {
    localStorage.setItem(INTRO_KEY, '1')
  } catch {
    // sin almacenamiento: no se puede recordar
  }
}

/** Si la intro va al abrir la app. */
export function introOnStart(): boolean {
  if (introPreview()) return true
  if (import.meta.env.MODE === 'test') return false
  return !introSeen()
}
