import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

/** Combina clases de Tailwind sin conflictos. Es el `cn` que importan los componentes de 21st.dev. */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}
