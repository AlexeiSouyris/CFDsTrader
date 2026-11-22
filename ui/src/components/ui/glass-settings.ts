/** Ajustes del vidrio del armazón (desenfoque y opacidad) y sus presets. */
export type GlassSettings = { blur: number; opacity: number }
export const DEFAULT_GLASS: GlassSettings = { blur: 22, opacity: 0.7 }
export const PRESETS: { name: string; value: GlassSettings }[] = [
  { name: 'Claro', value: { blur: 8, opacity: 0.22 } },
  { name: 'Esmerilado', value: DEFAULT_GLASS },
  { name: 'Sólido', value: { blur: 30, opacity: 0.88 } },
]
