---
modulo: fund
ruta: `engine/crates/fund` (crate `cfds-fund`)
lenguaje: Rust
estado: hecho
resumen: Fondo del grupo: aportes y retiros, unidades, valor de la unidad (NAV) y P&L por miembro.
---

# fund

## Responsabilidad (única)

- Fondo del grupo: aportes y retiros de cada miembro, unidades, valor de la unidad (NAV) y P&L por miembro. Hoy: liquidación de aportes y retiros al NAV del viernes con registro de cada movimiento, y estado de cuenta que suma exactamente el valor del fondo al céntimo (mayor resto)
- historial de solo agregar en PostgreSQL (`fund_movements`, `fund_movement.v1`) del que se reconstruyen las cuentas (`replay`, `PgFund`)
- resumen para la UI (`summary`, `fund_summary.v1`, #312): NAV, valor total y la cuenta de cada miembro, valuado al cerrar la última liquidación
- `config/fund.toml`

## Estado

- adelanto de la fase 6b (#177), historial (#179), resumen (#312)
