/**
 * El nombre legible de un instrumento (#861): "DAX 40 (Alemania)" en vez de "DE40". El nombre sale
 * del catálogo del motor (`gateway_instruments.v1`); el id queda chico al lado, para quien lo busque.
 * Sin nombre en el catálogo (IBKR, sin gateway), muestra el id.
 */
import { instrumentName, type InstrumentNames } from '@/lib/liquidity-api'

export function InstrumentName({
  id,
  names,
  showId = true,
  className,
}: {
  id: string
  names: InstrumentNames
  /** el id chico al lado del nombre */
  showId?: boolean
  className?: string
}) {
  const name = instrumentName(names, id)
  return (
    <span className={className} title={name === id ? undefined : id}>
      {name}
      {showId && name !== id && (
        <span
          data-instrument-id
          className="ml-1 text-[0.8em] font-normal tracking-normal normal-case opacity-70"
        >
          {id}
        </span>
      )}
    </span>
  )
}
