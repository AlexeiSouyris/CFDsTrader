/**
 * Las órdenes enviadas desde esta pestaña al broker simulado (#552), con el informe que mandó el
 * motor (`order.v1` → `order_report.v1`). El gateway todavía no tiene una ruta para listar las
 * órdenes (`/api/v1/paper/orders` solo acepta `POST`), así que la lista es de la sesión: se pierde
 * al recargar. Solo muestra lo que llegó; no calcula nada.
 */
import { ScrollRegion } from '@/components/ui/scroll-region'
import { dateTimeUtc, decimal } from '@/lib/format'
import { REJECTION_LABEL, type Order, type OrderReport } from '@/lib/paper-contracts'
import { cn } from '@/lib/utils'

export type SentOrder = { order: Order; report: OrderReport }

const STATUS: Record<OrderReport['status'], { label: string; tone: string }> = {
  filled: { label: 'Ejecutada', tone: 'bg-up/10 text-up' },
  pending: { label: 'Pendiente de la próxima vela', tone: 'bg-foreground/5 text-foreground' },
  rejected: { label: 'Rechazada', tone: 'bg-down/10 text-down' },
}

export function SessionOrders({ orders }: { orders: SentOrder[] }) {
  if (orders.length === 0)
    return (
      <p className="rounded-xl border border-dashed border-border px-4 py-6 text-center text-[13px] text-muted-foreground">
        <strong className="block text-[13.5px] font-bold text-foreground">
          Todavía no se envió ninguna orden desde esta pestaña.
        </strong>
        El gateway no tiene una ruta para listar las órdenes: acá se ven las que se envían desde el
        ticket, con la respuesta del broker simulado.
      </p>
    )
  const cell = 'px-2 py-2.5'
  return (
    <ScrollRegion label="Órdenes de la sesión">
      <table className="w-full min-w-[640px] text-[12.5px]">
        <caption className="sr-only">Órdenes de la sesión</caption>
        <thead>
          <tr className="border-b border-border text-left text-[10px] tracking-[0.1em] text-muted-foreground uppercase">
            <th scope="col" className={cell}>
              Hora
            </th>
            <th scope="col" className={cell}>
              Instrumento
            </th>
            <th scope="col" className={cell}>
              Estado
            </th>
            <th scope="col" className={cn(cell, 'text-right')}>
              Tamaño
            </th>
            <th scope="col" className={cn(cell, 'text-right')}>
              Precio
            </th>
            <th scope="col" className={cn(cell, 'text-right')}>
              Stop
            </th>
            <th scope="col" className={cell}>
              Orden
            </th>
          </tr>
        </thead>
        <tbody>
          {orders.map(({ order, report }) => {
            const st = STATUS[report.status]
            return (
              <tr
                key={order.order_id}
                data-status={report.status}
                className="border-b border-border last:border-b-0"
              >
                <td className={cn(cell, 'whitespace-nowrap text-muted-foreground tabular-nums')}>
                  {dateTimeUtc(report.at)}
                </td>
                <td className={cn(cell, 'whitespace-nowrap')}>
                  <span className="font-extrabold">{order.instrument_id}</span>{' '}
                  <span
                    className={cn(
                      'ml-1 rounded-md px-1.5 py-0.5 text-[10.5px] font-bold',
                      order.side === 'long' ? 'bg-up/10 text-up' : 'bg-down/10 text-down',
                    )}
                  >
                    {order.side === 'long' ? 'Compra' : 'Venta'}
                  </span>
                </td>
                <td className={cell}>
                  <span className={cn('rounded-md px-1.5 py-0.5 text-[11px] font-bold', st.tone)}>
                    {st.label}
                  </span>
                  {report.rejection && (
                    <span className="mt-1 block text-[11px] text-muted-foreground">
                      {REJECTION_LABEL[report.rejection]}
                    </span>
                  )}
                </td>
                <td className={cn(cell, 'text-right tabular-nums')}>
                  {report.size ? `${decimal(report.size)} €/pt` : '—'}
                </td>
                <td className={cn(cell, 'text-right tabular-nums')}>
                  {report.fill_price ? decimal(report.fill_price) : '—'}
                </td>
                <td className={cn(cell, 'text-right tabular-nums')}>{decimal(order.stop)}</td>
                <td
                  className={cn(cell, 'max-w-[10rem] truncate text-muted-foreground')}
                  title={order.order_id}
                >
                  {order.order_id}
                </td>
              </tr>
            )
          })}
        </tbody>
      </table>
    </ScrollRegion>
  )
}
