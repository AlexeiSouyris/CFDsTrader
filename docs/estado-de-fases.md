# Estado de las fases (para aprobar)

Actualizado el 24-11-2026. Es un resumen para el dueño: por cada fase de `docs/ROADMAP.md`, el criterio de aprobación, lo mergeado, la evidencia o lo que falta, cómo verificarlo y si la aprobación ya quedó registrada.

`CLAUDE.md` §5c pide que **ninguna fase empiece sin que la anterior esté aprobada y registrada en su Issue**. Mientras el dueño construya solo, alcanza con su aprobación explícita (modo de un solo constructor). La fase 11 siempre pide dos personas.

**Estados:** "aprobada" · "aprobada para avanzar" (el dueño autorizó seguir, pero su criterio de aprobación **no** está cumplido) · "listo para aprobar" · "bloqueado por …" (una decisión o un dato) · "falta trabajo".

**24-11-2026 ([#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419)):** el dueño escribió «te apruebo los 11 ya ahora para que hagas lo que necesites con todo mi permiso» y «Yo me refería a que te apruebo las 11 fases». Queda registrado así: las fases 4b, 5, 7, 8, 8b y 10 están **aprobadas para avanzar**, cada una con su criterio todavía pendiente (no se marca como cumplido). La fase 11 (dinero real) **no** queda aprobada: `CLAUDE.md` exige un ADR aprobado por dos personas, sin excepción en el modo de un solo constructor.

Los comandos se corren desde la raíz del repo. Los que leen la base necesitan `DATABASE_URL`. **Ninguno gasta cupo de IG.**

## Resumen

| Fase | Estado | Aprobación registrada |
|---|---|---|
| 1 Especificación | aprobada | sí (PR #1) |
| 2 Datos | aprobada | [sí, #2](https://github.com/AlexeiSouyris/CFDsTrader/issues/2#issuecomment-7052382999) |
| 3 Motor de backtest | aprobada | [sí, #74](https://github.com/AlexeiSouyris/CFDsTrader/issues/74#issuecomment-7052697748) |
| 3b Análisis técnico | aprobada | [sí, #36](https://github.com/AlexeiSouyris/CFDsTrader/issues/36#issuecomment-7054465898) |
| 4 Estrategia | aprobada | [sí, #86](https://github.com/AlexeiSouyris/CFDsTrader/issues/86#issuecomment-7056065342) |
| 4b Inteligencia | aprobada para avanzar; criterio pendiente (bloqueado por datos) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) (#99 abierto) |
| 5 Evaluación | aprobada para avanzar; criterio pendiente (bloqueado por datos y decisiones) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) |
| 6 Riesgo | aprobada | [sí, #350](https://github.com/AlexeiSouyris/CFDsTrader/issues/350) |
| 6b Fondo | aprobada | [sí, #351](https://github.com/AlexeiSouyris/CFDsTrader/issues/351) |
| 7 Broker | aprobada para avanzar; criterio pendiente (decisión de la cuenta demo) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) (#278 abierto) |
| 8 Paper trading | aprobada para avanzar; criterio pendiente (período de paper) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) |
| 8b Aprendizaje continuo | aprobada para avanzar; criterio pendiente (falta trabajo y datos) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) (#327 abierto) |
| 9 Observabilidad | aprobada | [sí, #352](https://github.com/AlexeiSouyris/CFDsTrader/issues/352) |
| 10 UI y app | aprobada para avanzar; criterio pendiente (falta trabajo) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) (#275 abierto) |
| 11 Dinero real | bloqueada (ADR de dos personas) | no (no alcanza con el dueño) |

> **Orden.** El ROADMAP dice que no se empieza una fase sin aprobar la anterior. Desde el 24-11-2026 ([#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419)) el dueño aprobó **avanzar** en las fases 4b, 5, 7, 8, 8b y 10, aunque sus criterios no estén cumplidos. Esa aprobación permite trabajar en ellas; no las da por cumplidas. Cada una se cierra cuando se cumpla su criterio (o cuando el dueño cambie el criterio en un PR aparte).

---

## Fase 1: Especificación

- **Criterio:** "dos personas que lean la spec harían exactamente las mismas operaciones".
- **Mergeado:** #1 (spec de la fase 1); después la completaron, entre otros, #133 (§9.1), #149 (§13), #170 (ML v1) y #232 (§5.4.1).
- **Verificar:** leer `docs/spec/fase-1-especificacion.md`.
- **Aprobación:** registrada en #1 (el ROADMAP la marca aprobada).
- **Estado:** **aprobada**.

## Fase 2: Datos

- **Criterio:** "tests que fallan con datos corruptos a propósito".
- **Mergeado:** #3 a #10, #16 y #28; después #89 (errores de datos) y #271 (saltos reales). La carga de historia de IG sigue en #102.
- **Verificar:** `cd engine && cargo test -p cfds-market-data --test corrupted_data`
- **Aprobación:** [registrada en #2](https://github.com/AlexeiSouyris/CFDsTrader/issues/2#issuecomment-7052382999) (20-11-2026).
- **Estado:** **aprobada**. Las diarias que faltan se cargan con el plan de [#102](https://github.com/AlexeiSouyris/CFDsTrader/issues/102#issuecomment-7113212308), después del 27-11.

## Fase 3: Motor de backtest

- **Criterio:** "comprar y mantener da un resultado calculable a mano".
- **Mergeado:** #51 a #59, #77, #78 y #182; después #274 y #279 (kill switch).
- **Verificar:** `cd engine && cargo test -p cfds-backtest --test runner buy_and_hold`
- **Aprobación:** [registrada en #74](https://github.com/AlexeiSouyris/CFDsTrader/issues/74#issuecomment-7052697748) (20-11-2026).
- **Estado:** **aprobada**.

## Fase 3b: Análisis técnico

- **Criterio:** "coincide con la referencia, ningún valor histórico cambia al agregar datos, y una muestra de operaciones revisada en el gráfico cumple las reglas".
- **Mergeado:** sub-Issues #37, #39, #41, #43 y #80 a #82; el visor es de #85.
- **Verificar:** `cd engine && cargo test -p cfds-indicators -p cfds-patterns -p cfds-liquidity`
- **Aprobación:** [registrada en #36](https://github.com/AlexeiSouyris/CFDsTrader/issues/36#issuecomment-7054465898) (21-11-2026).
- **Estado:** **aprobada**.

## Fase 4: Estrategia

- **Criterio:** "cada regla de la spec tiene al menos un test".
- **Mergeado:** #92 a #98; después #157 (ajuste de noticias).
- **Verificar:** `cd engine && cargo test -p cfds-strategy && cargo test -p cfds-backtest --test spec_matrix`
- **Aprobación:** [registrada en #86](https://github.com/AlexeiSouyris/CFDsTrader/issues/86#issuecomment-7056065342) (21-11-2026).
- **Estado:** **aprobada**.

## Fase 4b: Inteligencia (Python)

- **Criterio:** "sin fuga de información (*leakage*) entre períodos, y mejora medida fuera de muestra frente a no usar la capa".
- **Mergeado:**
  - noticias: #110, #112, #131, #163, #185, #188 y #198/#199;
  - análogos: #153 y #295;
  - ML v1: #207 a #212 y #215;
  - sentimiento: #216, #217 y #223;
  - el reporte de aprobación: #226 (#221).
- **Evidencia:** los cortes son sin fuga (purga y embargo, #208, con tests). **Falta la medición con datos:**
  - `phase4b run` corre `backtest_run` sobre velas de IG, y la base no tiene velas de 15m de IG (la tabla `candles` tiene una sola vela).
  - Hay interpretaciones de noticias solo desde el corte del modelo (2025-12-03).
  - La spec ML v1 (#158) está aprobada.
- **Verificar, cuando haya datos:** `cd intelligence && DATABASE_URL=... uv run python -m cfds_intel.phase4b run <INSTRUMENTO> <desde> <hasta> 10000 1 <run_id>`
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). El Issue #99 sigue abierto.
- **Criterio pendiente:** "mejora medida fuera de muestra frente a no usar la capa" (la parte "sin fuga" tiene tests, pero el criterio entero no se dio por cumplido).
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por datos.** Hacen falta velas de 15m de IG del período (gastan cupo), o decidir correr el reporte con las velas de los futuros (habría que adaptarlo).

## Fase 5: Evaluación y anti-sobreajuste

- **Criterio:** "se sostiene fuera de muestra y con costos realistas".
- **Mergeado:** métricas (#59); walk-forward, comprar y mantener por ventana y fuera de muestra aparte (#301); sensibilidad (#303); diario (#306); el ejemplo `walk_forward` (#310); el bloque en la UI (#318); el diagnóstico de DE40 (#324).
- **Evidencia y lo que falta:**
  - **DE40.FUT** tiene 19 meses de velas y `config/data_split.toml` pide al menos 39 (24 + 3 + 12). El ejemplo se detiene con `NotEnoughData`.
  - **US500, US100 y US30** tienen 5 años de futuros, pero su `funding_benchmark` está pendiente: sin la tasa de financiación, el backtest no corre.
  - El diagnóstico en sample (#324) muestra que hoy la estrategia **pierde en DE40 antes de costos**: aun con datos, el criterio probablemente no se cumpla sin cambios en la estrategia.
- **Verificar:** `cd engine && cargo run --release -p cfds-backtest --example walk_forward -- DE40 <desde> <hasta> 10000 1 <report_id> --fuente futuros`; en la UI, Performance → detalle de una corrida → "Walk-forward / fuera de muestra".
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). (#298 se cerró como trabajo, no como aprobación.)
- **Criterio pendiente:** "se sostiene fuera de muestra y con costos realistas".
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por decisiones** (tasa de US500, separación de DE40 y cambios de estrategia; ver la lista al final).

## Fase 6: Gestión de riesgo

- **Criterio:** "un test por límite que verifica que se dispara".
- **Mergeado:** #50 (#49); el kill switch y su reactivación solo en backtest (#274 y #279); el broker usa el riesgo como obligatorio (#284).
- **Evidencia:** `engine/crates/risk/tests/risk.rs` tiene un test por límite:
  - riesgo del 1 % y tope de apalancamiento;
  - kill switch, drawdown del 15 % y pérdida diaria del 3 %;
  - máximo de 5 posiciones y una por instrumento;
  - distancia del stop entre 0,5 y 3 ATR;
  - margen y mínimo del broker (confirmado y sin confirmar);
  - datos imposibles;
  - fuera del backtest, el kill switch no se reactiva solo.

  La orden duplicada la cubre la idempotencia del broker (`cfds-broker`, tests de `paper`).
  **Salvedad:** el "precio razonable" de la validación hoy solo rechaza precios imposibles (≤ 0, o el stop en la entrada). No hay un control de distancia al mercado.
- **Verificar:** `cd engine && cargo test -p cfds-risk --test risk && cargo test -p cfds-broker --test paper`
- **Aprobación:** registrada en #350 (24-11-2026), fuera de orden y con la salvedad del precio razonable aceptada.
- **Estado:** **aprobada**.

## Fase 6b: Fondo del grupo

- **Criterio:** "el ejemplo del glosario (Ana, Beto y Caro) da exactamente el resultado calculado a mano, y la suma de lo que vale cada miembro es igual al valor total del fondo".
- **Mergeado:** contabilidad por unidades (#178), historial en PostgreSQL (#180), el fondo real en la UI (#313).
- **Evidencia:** el test `glossary_example_comes_out_exact_through_the_route` da NAV 1,10, 275 € y 250 unidades, con lo de cada miembro igual al cálculo a mano. Hay otro test para repartos que no cierran en céntimos.
- **Verificar:** `cd engine && cargo test -p cfds-fund && cargo test -p cfds-gateway --test fund_routes`; en la UI, la vista del fondo.
- **Aprobación:** registrada en #351 (24-11-2026), fuera de orden.
- **Estado:** **aprobada**.

## Fase 7: Broker

- **Criterio:** "funciona igual contra el simulador y contra la cuenta demo del broker".
- **Mergeado:** contratos (#280); broker simulado (#284); reconciliación (#285); swap (#288); registro (#290); rutas (#291); salidas iguales al backtest (#325); cierre de fin de semana (#336).
- **Evidencia:** el simulador está completo y probado. **Contra la cuenta demo no se probó:**
  - `LiveBroker` es imposible de construir (el dinero real está bloqueado, CLAUDE.md §5);
  - el proveedor de IG solo lee precios (ADR 0002): no hay órdenes a la demo.
- **Verificar:** `cd engine && cargo test -p cfds-broker`
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). El Issue #278 sigue abierto.
- **Criterio pendiente:** "funciona igual contra el simulador y contra la cuenta demo del broker" (falta la cuenta demo).
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por decisión:** ¿se habilitan órdenes contra la cuenta **demo** de IG (sin dinero real, con un ADR), o se cambia el criterio para aprobar solo con el simulador?

## Fase 8: Paper trading

- **Criterio:** "la diferencia está explicada".
- **Mergeado:** runner (#308) y su modo en el gateway (#311); comparación paper contra backtest (#320 y #322); vistas en la UI (#335); cierre de fin de semana (#336). Abiertos: el diario de paper (#338 y #339).
- **Evidencia:** la comparación reparte la diferencia por causas y suma exacto (tests de `cfds-runner --test compare`). **Falta un período real de paper:**
  - la spec pide 3 meses;
  - el runner está apagado (`config/runner.toml`, `enabled = false`);
  - hacen falta las velas en vivo de IG (`ig_live`, gasta cupo).
- **Verificar:** `cd engine && DATABASE_URL=... cargo run -p cfds-gateway --example paper_vs_backtest -- DE40 <desde> <hasta>`; en la UI, Performance → "Paper vs backtest".
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419).
- **Criterio pendiente:** "la diferencia está explicada", sobre un período real de paper (la spec pide 3 meses).
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por decisión** (habilitar el runner) **y por tiempo** (el período de paper).

## Fase 8b: Aprendizaje continuo

- **Criterio:** "el informe se genera con los datos del paper trading, la calibración se puede verificar a mano con una muestra, y ningún parámetro cambia sin un voto registrado".
- **Mergeado:** probabilidad §13 con Wilson (#176); el informe con su muestra para verificar a mano (#329); la vista en la UI (#335). Abierto: propuestas de cambio y votos (#330).
- **Falta:** mergear #330 (votos), que el runner escriba el diario (#338 y #339) y **datos de paper** para generar el informe.
- **Verificar:** `cd engine && cargo test -p cfds-journal learning`; en la UI, Performance → "Aprendizaje".
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). El Issue #327 sigue abierto.
- **Criterio pendiente:** "el informe se genera con los datos del paper trading, la calibración se puede verificar a mano con una muestra, y ningún parámetro cambia sin un voto registrado".
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue faltando **trabajo** (#330, #338 y #339) **y datos de paper**.

## Fase 9: Observabilidad y operación segura

- **Criterio:** el ROADMAP **no escribe uno**. Pide logs estructurados, alertas (Telegram o email), y arranque y parada seguros sin órdenes huérfanas.
- **Mergeado:** logs JSON sin secretos y alertas (#300); arranque y parada seguros (#302); estado de la operación y del riesgo en la UI (#317).
- **Evidencia:** tests de `cfds-obs` y del gateway (`ops`, arranque y parada). Telegram es opcional y está apagado si no hay token.
- **Verificar:** `cd engine && cargo test -p cfds-obs -p cfds-gateway`; en la UI, la barra de estado.
- **Aprobación:** registrada en #352 (24-11-2026), fuera de orden, con este alcance como criterio.
- **Estado:** **aprobada**. Prender Telegram sigue a decisión del dueño.

## Fase 10: UI y app del celular

- **Criterio:** el ROADMAP **no escribe uno**. Pide la app privada para los 6 iPhone, con push, aprobación de propuestas, votaciones, cuentas con 2FA y P&L, gráficos, noticias, parámetros, la vista del fondo y el diccionario.
- **Mergeado, entre otros:** prototipo (#31, #165 y #172); liquidez (#233 y #244); pestañas (#289); terminal paper (#276 y #293); PWA instalable (#309); fondo (#313); estado (#317); vistas de las fases 8 y 8b (#335).
- **Falta:** cuentas con 2FA, notificaciones push, votaciones y acceso desde el celular fuera de esta computadora (el gateway escucha en `127.0.0.1`).
- **Verificar:** `cd ui && npm test`; abrir la UI con `infra/scripts/ui_liquidez.sh`.
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). El Issue #275 sigue abierto.
- **Criterio pendiente:** el ROADMAP no escribe uno; queda pendiente el alcance que falta: cuentas con 2FA, notificaciones push, votaciones y acceso desde el celular fuera de esta computadora.
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue faltando **trabajo**.

## Fase 11: Dinero real

- **Criterio:** "solo con ADR aprobado por ambos, después de un paper trading aprobado, y empezando con capital mínimo".
- **Aprobación:** no registrada. La aprobación general del dueño del 24-11-2026 ([#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419)) **no** alcanza: `CLAUDE.md` §5 y el modo de un solo constructor exigen un ADR aprobado por dos personas.
- **Estado:** **bloqueada.** Pide dos personas y la fase 8 aprobada. `LiveBroker` es imposible de construir a propósito.

---

## Decisiones pendientes del dueño

1. **Tasa de financiación de US500, US100 y US30** (`funding_benchmark` pendiente en `config/instruments.toml`: ¿es €STR, como en DE40?). Confirmarlo con IG desbloquea sus backtests y la fase 5 en EE. UU.
2. **DE40: separación de datos o más historia.** `data_split.toml` pide 39 meses y hay 19 de futuros. Las salidas son más historia de FDAX o una separación distinta para la evaluación (también afecta a ML v1, §5).
3. **Cambios de estrategia** después del diagnóstico (#324): horario de apertura, distancia del stop, TP1 o break-even, peso de "patrón de velas + rechazo en nivel". Se evalúan con walk-forward antes de tocar la config.
4. **Habilitar el runner de paper** (`config/runner.toml`, `enabled`): es lo que arranca el período de paper de la fase 8.
5. **`CFDS_GATEWAY_TOKEN`:** definirlo en el `.env` del Mac mini habilita las escrituras de paper desde la UI. Sin él, la UI es de solo lectura.
6. **La ruta del crontab:** confirmar la carpeta absoluta para las tareas de `cron` (`docs/guides/puesta-en-marcha-local.md`; `cron` no expande `~` ni `$HOME`).
7. **Acceso desde la red local o el celular:** el gateway escucha en `127.0.0.1:8787` (`config/gateway.toml`). Abrirlo pide decidir cómo, por ejemplo LAN con el token o una VPN.
8. **Telegram:** las alertas están listas (`config/alerts.toml`); falta decidir si se activan y crear el bot (`TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID`).
9. **Cuentas para votar:** quién vota y cómo se identifica cada uno (spec §1.2). Hace falta para las votaciones de la fase 8b y para las cuentas con 2FA de la fase 10.
