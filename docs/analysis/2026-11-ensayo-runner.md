# Ensayo del runner de paper sobre velas guardadas (#406)

Fecha: 2026-11-24. Antes de prender el runner el 27-11 (#380, PR #382).

**Resumen.** El runner decide igual que el backtest en el tramo ensayado: 79 de 79 decisiones iguales, y con fills 26 de 26 operaciones emparejadas, con el mismo precio de entrada. El kill switch por hueco salta a tiempo y un reinicio corto no duplica nada. Pero hay **dos bloqueantes para el 27-11** y tres errores más. Cada uno tiene su Issue:

| # | Hallazgo | ¿Bloquea el 27-11? |
|---|---|---|
| #437 | `ig_live` no guarda el spread de cierre: el filtro de spread bloquea **toda** entrada y el runner nunca propone | **Sí** |
| #438 | Un reinicio después de 10 días de registro da drifts en la reconciliación: kill switch, compuerta cerrada y una alerta del diario en cada paso | **Sí**, si el gateway se reinicia después del 07-12 |
| #439 | Las tasas de financiación (€STR) se cargan solo al arrancar: el swap se desvía y, al reiniciar, los informes cambian | No, pero alimenta #438 |
| #441 | El kill switch no sobrevive a un reinicio: el sistema vuelve a operar sin la votación de §1.2 | No para proponer; sí para la seguridad |
| #440 | `paper_vs_backtest`: el backtest opera durante el calentamiento y la diferencia sale como «tamaño» | No; la comparación de la fase 8 sale mal |

Además hay que decidir el **capital de paper**. Con los 1000 € de `config/broker.toml`, el riesgo rechaza las 79 propuestas del DE40: nada se ejecuta y el diario queda vacío.

## Cómo se hizo

- **Base de pruebas propia** `cfds_test_runner`, migrada con `cfds-storage --example migrate` hasta la 0037. Se borró al terminar. **Nada se escribió en la base real.** Desde ella solo se copió, en sesión de solo lectura:
  - las velas de 15m y 1h del futuro del DAX (`futures_candles`, Databento, `FDAX.c.0`, un solo contrato desde el 22-09) del 22-09 al 30-10, como velas `DE40` en `candles` (2300 de 15m y 527 de 1h);
  - `instruments`, `reference_rates` (€STR), `economic_events`, `news_items` y `news_interpretations`.
- **Por qué el DE40 y con futuros.** La tabla `candles` real tiene **una sola** vela del DE40 de IG (23-11 08:00), así que no hay tramo de IG para ensayar. US500, US100 y US30 no sirven: el broker simulado los saltea porque su `funding_benchmark` está pendiente en el catálogo. En paper **solo se opera el DE40**.
- **Spreads sintéticos.** `candle_spreads` está vacía en la base real (ver #437). Para que el filtro de spread deje entrar, la base de prueba lleva un spread constante de 1,8 puntos (el típico del catálogo) en cada vela de 15m, con fuente `ensayo_406_sintetico`. Un spread constante nunca supera 2 veces su promedio: el filtro solo exige las muestras mínimas.
- **Reloj simulado.** Nuevo ejemplo `engine/crates/gateway/examples/runner_replay.rs`. Corre el **mismo** `PgPaper::start` → `PgPaper::run_once` → `PgPaper::shutdown` del gateway, con la config de `config/` y la sesión de PostgreSQL de solo lectura del gateway. Solo cambia `Utc::now()` por el cierre de cada vela de 15m más 60 s. No toca la lógica de decisión. Se niega a correr si la base no es `cfds_test_*`. Las alertas van solo al log, sin Telegram.
- **Comparación.** `paper_vs_backtest` sobre la misma base. Ahora también compara **todas las decisiones**, no solo las operaciones: cada orden del registro, llenada o rechazada, contra la señal del backtest en la misma vela y lado.
- Tramo principal: 28-09 00:00 → 31-10 00:00 UTC, 3169 marcas del reloj y 1990 decisiones.

## Resultados por escenario

### A. Config tal cual (capital de paper 1000 €)

- 79 propuestas, **79 rechazadas por el riesgo**: 40 `below_min_deal_size` (con 10 € de riesgo por operación y stops de 20 a 120 puntos, el tamaño queda por debajo del mínimo de 0,5) y 39 `stop_too_far` (stop a más de 3 ATR).
- `paper_vs_backtest`: 0 operaciones de cada lado, y **79 de 79 decisiones iguales** (misma vela, mismo lado y mismo motivo de rechazo).
- El diario no guarda nada: solo se registran las propuestas **llenadas** (`proposals` 0). El informe de aprendizaje no ve los rechazos.
- Una alerta `rejected_streak` (5 rechazos seguidos) el primer día.

### B. Capital de prueba 100 000 € (solo en la compilación local del ensayo, sin commitear)

- 79 propuestas: 26 llenadas, 28 `stop_too_far` y 25 `instrument_already_open`. Capital final 92 369,37 €.
- **Diario completo:** 26 `proposal.v1` (fuente `paper`, `run_id = run`, probabilidad mostrada «datos insuficientes», 0 de 30 casos, porque no hay `probability_run_id`) y 26 `outcome.v1`, todos cerrados con velas cerradas.
- `paper_vs_backtest --guardar`: **26 emparejadas, 0 faltan y 0 sobran**, con el mismo precio de entrada en todas. Diferencia 133,00 € = 131,71 € «tamaño» + 1,29 € «swap». La comparación quedó guardada en `paper_comparisons`.
  - El «tamaño» es un **error de la comparación** (#440). El backtest arranca 15 días antes para el calentamiento, opera en ese tramo y llega al 28-09 con 101 743 €. La relación de tamaños es 0,98287 en las 26 operaciones.
  - El «swap» viene de #439: paper usó la €STR conocida al arrancar y la reconstrucción, la del 31-10. Por eso también avisa «18 informes de paper no salen iguales al reconstruirlos».
- Decisiones: 54 iguales. Las 25 restantes son los rechazos `instrument_already_open` de paper: el backtest ni evalúa mientras tiene una posición abierta. No es una diferencia de operaciones.
- 33 liquidaciones diarias de swap en el log, una por día.

### C. Reinicio a mitad, después de 16 días (28-09 → 14-10 12:00, reinicio, → 31-10)

- Hasta el reinicio: órdenes, informes, propuestas y resultados **idénticos** a B. Después del reinicio, **ninguna orden duplicada**: la decisión de las 12:00 se repite a propósito y el broker devuelve el informe guardado.
- Al arrancar: `startup_reconcile clean=false, drifts=28`. El gateway carga solo 10 días de velas (`[paper] candles_days`), las órdenes de antes del 04-10 salen `no_price` y la reconciliación activa el **kill switch**. La compuerta queda en `Starting` y el runner pasa **1585 pasos seguidos en «compuerta cerrada»**: no propone nada más hasta el 31-10.
- Cada paso intenta reescribir los resultados ya guardados con la valuación reconstruida: **14 265 `outcome_write_failed`** y una alerta de desconexión en cada paso (#438).

### D. Hueco inyectado (4 velas de 15m y la de 1h del 14-10, 10:00–11:00 UTC)

- A las 11:01 salta el aviso «faltan 1 velas de 15m de DE40 desde 10:00 (datos en vivo)» y se activa el **kill switch**. Cuando llega la vela siguiente, la validación marca además el error de datos («error de datos en DE40 desde 10:00»). Las **117 decisiones** siguientes se saltean con «kill switch activo». ✅
- **Con un reinicio** a las 13:00 (D1 → D2), el gateway arranca limpio y **sin kill switch**. El hueco ya está en la ventana al arrancar, así que no lo vuelve a disparar, y el runner vuelve a decidir (#441).
- Mientras el hueco esté en la ventana de 15 días, el aviso `data_issues` se repite en **cada** paso: 141 líneas en 35 horas simuladas. En vivo serían 2 por minuto durante 15 días.

### E. Sin spreads guardados (como hoy con `ig_live`)

- 28-09 → 10-10: 785 decisiones, **0 propuestas**. Todas salen `no_signal` porque `spread_flags` da `false` sin spread de la vela (#437).

### F. Reinicio corto (28-09 → 30-09 12:00, reinicio, → 03-10)

- Reconciliación al arrancar **limpia** (las velas cargadas cubren todo el registro). Órdenes (19), informes, propuestas (8) y resultados iguales a B y **sin duplicados**. ✅
- La única diferencia es el swap de la operación que cruzó el reinicio (`run:DE40:2026-09-30T14:30:00Z:short`): 2,3475 € sin reinicio y 2,3517 € con reinicio, por #439.

## Qué hay que arreglar o decidir antes del 27-11

1. **#437, bloqueante.** Que `ig_live` guarde el spread de cierre (IG ya lo manda en el mismo pedido: `history_with_spreads`, #95). Aun así, con `min_samples = 10` por franja horaria y `lookback_days = 20`, la primera entrada posible llega **unas dos semanas** después de empezar a guardar spreads. Si se quiere proponer antes, hay que decidir qué hace el filtro sin muestras: hoy bloquea, y es lo conservador.
2. **Capital de paper**, decisión del dueño. Con 1000 € y riesgo del 1 %, el DE40 rechaza todo. Hay tres salidas: subir `initial_equity`, aceptar que paper solo registre rechazos o revisar el mínimo. Además, el diario solo guarda las propuestas llenadas: los rechazos quedan en el log y en `paper_orders`, no en `proposals`.
3. **#438, bloqueante a los 10 días.** Hasta que se arregle, **no reiniciar el gateway** después de 10 días de operación del runner. Si hace falta, subir `[paper] candles_days` lo posterga, pero no lo arregla.
4. **Calentamiento.** La tabla `candles` real tiene una sola vela del DE40. El runner necesita 200 velas de 15m y 60 de 1h en sesión (`config/strategy.toml`): unos 3 días de `ig_live` antes de la primera decisión. Con #437 resuelto, el plazo lo marcan los spreads.
5. **#439, #441 y #440:** no impiden prender el runner. Sí hay que arreglarlos antes de dar por aprobada la fase 8 con `paper_vs_backtest` y antes de confiar en el kill switch después de un reinicio.

## Otras observaciones (sin Issue)

- **Costo por paso.** Cada paso cuesta más a medida que se acumulan velas y operaciones: el broker revalúa todo su registro y la reconciliación periódica lo reconstruye. Con la máquina cargada (carga media de 10 a 18) y en compilación release, se pasó de 1–5 s por día simulado en la primera semana a 70–90 s en la quinta (unos 0,8 s por paso). Con `poll_secs = 30` hay margen, pero crece sin límite y el candado del broker lo comparten las rutas de la UI.
- **Tamaños sin redondear.** Los tamaños tienen 27 decimales (por ejemplo, 8,539279443…). IG opera con un incremento de tamaño. El backtest hace lo mismo, así que la comparación no lo ve. Conviene revisarlo antes de pasar a una demo real.
- **Alertas `rejected_streak`** (5 rechazos seguidos): con el capital actual saltarían casi a diario.
- Al primer paso de cada arranque, la decisión sobre la última vela del viernes sale «datos viejos». Es correcto.

## Cómo repetirlo

```text
# base propia (nunca la real); el usuario sale de DATABASE_URL sin imprimirlo
docker exec cfdstrader-postgres-1 createdb -U <usuario> cfds_test_runner
DATABASE_URL=<…/cfds_test_runner> cargo run -q -p cfds-storage --example migrate
# copiar velas, tasas, calendario y noticias desde la real con COPY (sesión de solo lectura)
# y, si hace falta, spreads sintéticos en candle_spreads
DATABASE_URL=<…/cfds_test_runner> cargo run --release -p cfds-gateway --example runner_replay -- \
    2026-09-28T00:00:00Z 2026-10-31T00:00:00Z 2> runner.log
DATABASE_URL=<…/cfds_test_runner> cargo run --release -p cfds-gateway --example paper_vs_backtest -- \
    DE40 2026-09-28T00:00:00Z 2026-10-31T00:00:00Z
docker exec cfdstrader-postgres-1 dropdb -U <usuario> cfds_test_runner
```

El reinicio se ensaya corriendo `runner_replay` en dos tramos seguidos (`desde → medio` y `medio → hasta`), y el hueco, borrando velas en la base de prueba.
