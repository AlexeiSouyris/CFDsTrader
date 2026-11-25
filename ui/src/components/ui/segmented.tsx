/** Selector de opciones en una píldora (rango, tipo de gráfico). Uno solo para toda la app. */
import { cn } from '@/lib/utils'

export function Segmented({
  label,
  options,
  value,
  onChange,
  format,
}: {
  label: string
  options: readonly string[]
  value: string
  onChange: (v: string) => void
  /** el texto de cada opción, si no es el valor mismo */
  format?: (o: string) => string
}) {
  return (
    <div role="group" aria-label={label} className="flex rounded-full border border-border p-0.5">
      {options.map((o) => (
        <button
          key={o}
          type="button"
          aria-pressed={o === value}
          onClick={() => onChange(o)}
          className={cn(
            'rounded-full px-2.5 py-1 text-[11px] font-bold transition-colors',
            o === value
              ? 'bg-primary text-primary-foreground'
              : 'text-muted-foreground hover:text-foreground',
          )}
        >
          {format ? format(o) : o}
        </button>
      ))}
    </div>
  )
}
