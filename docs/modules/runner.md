---
modulo: runner
ruta: `engine/crates/runner` (crate `cfds-runner`)
lenguaje: Rust
estado: hecho
resumen: Runner de paper trading en tiempo real sobre velas cerradas ya guardadas (fase 8).
---

# runner

## Responsabilidad (única)

- Runner de paper trading (fase 8, #305): por instrumento y en tiempo real, sobre velas **cerradas ya guardadas** (nunca llama a IG), valida la serie (`check_series`), activa el kill switch y alerta ante un error de datos nuevo o velas que faltan en sesión, liquida el swap a diario (posiciones al corte de financiación, al log) y decide con la estrategia de siempre (`cfds_backtest::choose`, funciones puras, sin cambiar parámetros) en el cierre de `t`, viendo solo velas hasta `t`. Propone la orden con id idempotente `<prefijo>:<instrumento>:<cierre de t>` y `created_at` = cierre de `t`, que el broker simulado llena en la apertura de `t+1`
- no la envía (quien lo usa la manda con `submit_and_record`). Respeta la compuerta y el kill switch. Config en `config/runner.toml` (apagado por defecto). Diario de paper (`journal`, #333): la propuesta ejecutada como `proposal.v1` con la probabilidad mostrada, con id idempotente que lleva la orden (`proposal_id`), y su `outcome.v1` solo cuando la operación cerró con velas ya cerradas. Comparación paper vs backtest del mismo período (`compare`, `paper_comparison.v1`, #315): empareja por vela de decisión y reparte la diferencia por causa

## Estado

- núcleo del runner y modo del gateway (#305)
- comparación con el backtest (#315)
