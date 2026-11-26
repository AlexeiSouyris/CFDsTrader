/**
 * Sección "Operación" del Panel (#314): el estado del broker simulado y los últimos eventos y
 * alertas, tal como los manda el gateway (`ops_status.v1`, `ops_event.v1[]`). Solo lectura: el
 * kill switch no se reinicia desde acá. Sin gateway se dice, sin inventar un estado.
 */
import { OpsEventFeed, OpsKillSwitchBanner, OpsStatusBar } from '@/components/ui/ops-status'
import { useOpsEvents, useOpsStatus } from '@/lib/ops-api'

export function OperationView() {
  const status = useOpsStatus()
  const events = useOpsEvents()
  const live = status.status === 'ok' ? status.data : null
  return (
    <div className="grid gap-3">
      {live && <OpsKillSwitchBanner status={live} />}
      <OpsStatusBar status={live} />
      <section className="panel min-w-0 rounded-section p-4" aria-labelledby="ops-eventos">
        <h3 id="ops-eventos" className="text-[14px] font-bold">
          Eventos del broker y alertas
        </h3>
        <p className="mb-3 text-[11px] text-muted-foreground">
          Los últimos, del más nuevo al más viejo, tal como los registra el motor (sin secretos). Se
          guardan en la base y se conservan al reiniciar el gateway.
        </p>
        {events.status === 'ok' ? (
          <OpsEventFeed events={events.data} />
        ) : events.status === 'loading' ? (
          <p role="status" className="text-[12.5px]">
            Pidiendo los eventos al motor…
          </p>
        ) : (
          <p role="status" className="text-[12.5px]">
            Sin conexión con el gateway: no hay eventos para mostrar.
          </p>
        )}
      </section>
    </div>
  )
}
