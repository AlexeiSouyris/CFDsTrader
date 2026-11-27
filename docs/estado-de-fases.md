# Estado de las fases (para aprobar)

Actualizado el 24-11-2026, con lo mergeado esa noche (#451) y en la madrugada (#458 a #474; ver `docs/analysis/2026-11-24-resumen-noche.md`). Es un resumen para el dueño: por cada fase de `docs/ROADMAP.md`, el criterio de aprobación, lo mergeado, la evidencia o lo que falta, cómo verificarlo y si la aprobación ya quedó registrada.

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
| 8b Aprendizaje continuo | aprobada para avanzar; criterio pendiente (faltan datos de paper) | para avanzar, [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419) (#327 abierto) |
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
  - el reporte de aprobación: #226 (#221);
  - ML v1 en US500 con los futuros 2011–2026: etiqueta sin P/L (#403), pipeline e informe (#402), resultado (#430);
  - ML v2, tres variantes pre-registradas con corrección de Bonferroni (#435);
  - los experimentos y su calibración por grupos en Performance, tarjeta «Modelo (informativo)» (#434 y #442).
- **Evidencia:** los cortes son sin fuga (purga y embargo, #208, con tests).
  - **ML v1** (`docs/analysis/2026-11-ml-v1-us500.md`): 41 de 51 ventanas, 2639 propuestas de validación. M1 tiene AUC 0,525 y M1 − B1 en log loss da −0,0006 (IC 95 % de −0,0046 a +0,0039): **se queda B1**.
  - **ML v2** (`docs/analysis/2026-11-ml-v2-us500.md`, regla fijada antes en `docs/analysis/2026-11-ml-v2-preregistro.md`): tasas del Tesoro, VIX y análogos. **Ninguna mejora a B1** (AUC 0,502, 0,508 y 0,522).
  - El fuera de muestra **no se usó**: la spec lo reserva para un modelo que haya ganado en validación. Por ahora, ninguno ganó.
  - **Falta la medición de la capa entera con datos de IG:**
    - `phase4b run` corre `backtest_run` sobre velas de IG, y la base no tiene velas de 15m de IG (la tabla `candles` tiene una sola vela).
    - Hay interpretaciones de noticias solo desde el corte del modelo (2025-12-03).
    - La spec ML v1 (#158) está aprobada.
- **Verificar, cuando haya datos:** `cd intelligence && DATABASE_URL=... uv run python -m cfds_intel.phase4b run <INSTRUMENTO> <desde> <hasta> 10000 1 <run_id>`
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). El Issue #99 sigue abierto.
- **Criterio pendiente:** "mejora medida fuera de muestra frente a no usar la capa" (la parte "sin fuga" tiene tests, pero el criterio entero no se dio por cumplido).
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por datos.** Hacen falta velas de 15m de IG del período (gastan cupo), o decidir correr el reporte con las velas de los futuros (habría que adaptarlo). Con lo medido hasta ahora en US500, el ML no mejora a B1: cambiar el resultado pide cambiar la pregunta (más índices o qué se predice), y eso es un cambio de spec que decide el dueño.

## Fase 5: Evaluación y anti-sobreajuste

- **Criterio:** "se sostiene fuera de muestra y con costos realistas".
- **Mergeado:** métricas (#59); walk-forward, comprar y mantener por ventana y fuera de muestra aparte (#301); sensibilidad (#303); diario (#306); el ejemplo `walk_forward` (#310); el bloque en la UI (#318); el diagnóstico de DE40 (#324); la tasa administrativa del 3 % (#393); el estudio de variantes del DE40 (#373); las propuestas por mes de US500 con futuros 2011–2026 (#398); el estudio de financiación €STR/SOFR de EE. UU. (#411); Russell 2000 y US100 completo en walk-forward (#457); el diagnóstico de EE. UU. con futuros 2011–2025 (#461); la base de los futuros en la financiación y las tasas USD de la NY Fed (#470, ADR 0015); la propuesta de investigación pre-registrada (#466) y su ronda 1 (#474).
- **Evidencia y lo que falta:**
  - **DE40.FUT** tiene 19 meses de velas y `config/data_split.toml` pide al menos 39 (24 + 3 + 12). El ejemplo se detiene con `NotEnoughData`.
  - **US500, US100 y US30** tienen historia de futuros, pero su `funding_benchmark` sigue pendiente en `config/instruments.toml`: sin la tasa de financiación, el backtest oficial no corre.
  - **Estudio de financiación (#411, `docs/analysis/2026-11-wf-us-escenarios-financiacion.md`):** se corrió el walk-forward de los tres con dos tasas posibles, €STR y SOFR, solo en memoria y sin tocar el fuera de muestra. **El veredicto no cambia con la tasa:** US500 y US30 pierden con las dos, y US100 queda en torno a cero. No es un resultado oficial: la tasa la tiene que confirmar IG. US100 tiene la mitad de las ventanas porque sus futuros (NQ) empiezan en 11-2021.
  - El diagnóstico en sample (#324) muestra que hoy la estrategia **pierde en DE40 antes de costos**: aun con datos, el criterio probablemente no se cumpla sin cambios en la estrategia.
  - **Diagnóstico de EE. UU. (#461, `docs/analysis/2026-11-diagnostico-us-futuros.md`):** US500, US100 y US30 con futuros de 2011-02 a 2025-11, en 59 bloques trimestrales y en muestra.
    - Pierden los tres: −0,19 R, −0,12 R y −0,16 R netos por operación.
    - Antes de costos no hay ventaja (−0,02, +0,01 y −0,05 R); los costos se llevan 0,18, 0,13 y 0,10 R por operación.
    - Desde 2020, US100 queda cerca de cero neto (−0,01 R); US500 y US30 siguen perdiendo.
    - Ningún filtro (hora, día, gatillo, tendencia, volatilidad) da vuelta el signo de forma consistente en los tres índices. Son unas 300 comparaciones en muestra: cualquier hipótesis de ahí tiene que pre-registrarse.
  - **Base de los futuros (#470, ADR 0015):** con velas de futuros, el backtest ya no cuenta dos veces la tasa en la financiación (el doble conteo lo describió #468). En la estrategia actual el cambio es de unos 0,0015 R por operación.
    - Las tasas USD de la NY Fed (EFFR desde 2010-11 y SOFR desde 2018-04-02) se cargan con el mismo `python -m cfds_intel.rates`.
    - `config/candle_sources.toml` usa `mode = "rate"`, que supone que IG ajusta los dividendos en los CFD de índices. Está marcado **PENDIENTE para IG España**: hay que confirmarlo con IG.
  - **Propuesta de investigación (#466, `docs/analysis/2026-11-propuesta-investigacion.md`):** cinco familias de estrategias nuevas con fuente, una regla de decisión pre-registrada común (walk-forward, Bonferroni, Sharpe deflactado, consistencia, costos × 1,5) y el fuera de muestra intocable. Ronda 1 = ruptura del rango de apertura (C) y cambio de mes (D1).
  - **Ronda 1 (#474, `docs/analysis/2026-11-ronda1-orb-cambio-de-mes.md`): ninguna de las dos pasa y las dos se cierran.**
    - C pierde en la cartera y en los tres índices: −0,50 % por mes, t = −2,75.
    - D1 da +0,03 % por mes, no significativo (p ≈ 0,29–0,31, Sharpe deflactado ≈ 0,01), con la primera mitad (2013–2019) negativa. Solo pasa el criterio de costos × 1,5.
    - El fuera de muestra no se cargó.
    - Según la regla de #466 §5, **toca parar y revisar el planteo con el dueño** antes de la ronda 2 (decisión 13).
- **Verificar:** `cd engine && cargo run --release -p cfds-backtest --example walk_forward -- DE40 <desde> <hasta> 10000 1 <report_id> --fuente futuros`; en la UI, Performance → detalle de una corrida → "Walk-forward / fuera de muestra".
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). (#298 se cerró como trabajo, no como aprobación.)
- **Criterio pendiente:** "se sostiene fuera de muestra y con costos realistas".
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por decisiones** (tasa de US500, separación de DE40, cambios de estrategia y cómo seguir con la investigación; ver la lista al final).

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
- **Oro (XAUUSD, ADR 0013, #418), solo en paper:** ya están la clase `commodity` (#422), la sesión de 23 h de CME (#428), la fórmula de financiación de IG para materias primas (#429), el EUR/USD del BCE (#431), XAUUSD en el catálogo con tope 10:1 (#436) y la conversión con el tipo vigente en el backtest (#445). El broker simulado también convierte con el tipo vigente (#450): sin tipo publicado a la hora de la señal, rechaza la orden (`missing_fx_rate`). Las **horas de entrada** ya están: de 07:00 a 16:00 UTC (Londres + Nueva York), elegidas por el dueño el 25-11-2026 en #418 («dale, la (a) para el oro»), en `config/trading_window.toml`. **Hoy el oro no opera.** Falta:
  1. los **datos de IG** que siguen `pending` en `config/instruments.toml`: epic, spread típico, valor por punto, tamaño mínimo, tick y horario real;
  2. la **base de la financiación**: qué futuros usa IG, qué lado la paga y cómo cuenta los fines de semana;
  3. la **comisión de IG por convertir USD a EUR**;
  4. con eso, el estudio en backtest y walk-forward del ADR.
- **Estado:** **aprobada para avanzar; criterio pendiente.** Sigue **bloqueado por decisión:** ¿se habilitan órdenes contra la cuenta **demo** de IG (sin dinero real, con un ADR), o se cambia el criterio para aprobar solo con el simulador?

## Fase 8: Paper trading

- **Criterio:** "la diferencia está explicada".
- **Mergeado:** runner (#308) y su modo en el gateway (#311); comparación paper contra backtest (#320 y #322); vistas en la UI (#335); cierre de fin de semana (#336); el diario de paper (#338 y #339); el ensayo del runner sobre velas guardadas (#443); `ig_live` guarda el spread de cada vela (#446); el backtest de la comparación ya no opera en el calentamiento (#444); el kill switch de paper se guarda y sobrevive a un reinicio (#458); reinicio después de 10 días sin drifts (#459); la €STR y el EUR/USD se vuelven a leer a medida que se publican (#460); las lecturas de la cuenta no mueven el máximo del capital (#465); puntos de control del broker simulado (#469); la orden manual queda pendiente de su vela (#473).
- **Evidencia:** la comparación reparte la diferencia por causas y suma exacto (tests de `cfds-runner --test compare`).
  - **Ensayo del runner (#443, `docs/analysis/2026-11-ensayo-runner.md`):** con velas del futuro del DAX como DE40 (22-09 al 30-10) y spreads sintéticos, 79 de 79 decisiones iguales al backtest y 26 de 26 operaciones emparejadas. El kill switch por un hueco salta a tiempo y un reinicio corto no duplica nada.
  - **Lo que encontró el ensayo:**
    - #437, sin spreads guardados el runner nunca propone: **arreglado** (#446). El filtro pide 10 muestras por franja de una hora, y con velas de 15m alcanza con unas 3 sesiones de `ig_live` guardando spreads.
    - #440, el backtest de la comparación operaba en el calentamiento: **arreglado** (#444).
    - #438, un reinicio después de 10 días de registro deja el gateway con kill switch y la compuerta cerrada: **arreglado** (#459).
    - #439, las tasas de financiación se cargan solo al arrancar: **arreglado** (#460).
    - #441, el kill switch no sobrevive a un reinicio: **arreglado** (#458). Ahora queda guardado en `paper_kill_switch` y el motor nunca lo libera: lo libera una persona a mano, con el gateway apagado (ver la guía de uso, sección 5).
  - **Después del ensayo:**
    - #463, la reconstrucción del broker crecía sin límite con el registro: **arreglado** con puntos de control verificables (#469, tabla `paper_broker_checkpoints`, de solo agregar). No cambia ninguna decisión ni límite de riesgo.
    - #471, toda orden manual en vivo salía `no_price` (su vela del fill todavía no existía) y la reconstrucción la llenaba, con drift y kill switch: **arreglado** (#473). Si el riesgo la aceptaría, la orden queda **pendiente** hasta que llega su vela y recién ahí se decide; la UI la muestra como "Pendiente de la próxima vela". Si el gateway se apaga antes, la pendiente se pierde y se avisa.
  - **Capital de paper:** con los 1000 € de `config/broker.toml`, el riesgo rechazó las 79 propuestas del ensayo. Con ese capital no se ejecuta nada y el diario queda vacío. El resumen de la noche recomienda 10 000 € simulados; la decisión es del dueño (decisión 10).
  - **Falta un período real de paper:**
    - la spec pide 3 meses;
    - el runner está apagado (`config/runner.toml`, `enabled = false`; prenderlo es el PR #382, abierto);
    - hacen falta las velas en vivo de IG (`ig_live`, gasta cupo).
- **Verificar:** `cd engine && DATABASE_URL=... cargo run -p cfds-gateway --example paper_vs_backtest -- DE40 <desde> <hasta>`; en la UI, Performance → "Paper vs backtest".
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419).
- **Criterio pendiente:** "la diferencia está explicada", sobre un período real de paper (la spec pide 3 meses).
- **Estado:** **aprobada para avanzar; criterio pendiente.** Los errores que encontró el ensayo están arreglados. Sigue **bloqueado por decisión** (habilitar el runner y el capital de paper) **y por tiempo** (el período de paper, que puede empezar cuando se renueve el cupo de IG, el 27-11).

## Fase 8b: Aprendizaje continuo

- **Criterio:** "el informe se genera con los datos del paper trading, la calibración se puede verificar a mano con una muestra, y ningún parámetro cambia sin un voto registrado".
- **Mergeado:** probabilidad §13 con Wilson (#176); el informe con su muestra para verificar a mano (#329); la vista en la UI (#335); propuestas de cambio y votos (#330); el runner escribe propuestas y resultados en el diario (#338 y #339).
- **Falta:** **datos de paper** para generar el informe (el runner está apagado; ver la fase 8). Votar desde la pantalla no existe todavía: el gateway muestra el estado de cada propuesta, pero no tiene rutas para votar.
- **Verificar:** `cd engine && cargo test -p cfds-journal learning`; en la UI, Performance → "Aprendizaje".
- **Aprobación:** aprobada por el dueño para avanzar (24-11-2026), [#419](https://github.com/AlexeiSouyris/CFDsTrader/issues/419). El Issue #327 sigue abierto.
- **Criterio pendiente:** "el informe se genera con los datos del paper trading, la calibración se puede verificar a mano con una muestra, y ningún parámetro cambia sin un voto registrado".
- **Estado:** **aprobada para avanzar; criterio pendiente.** Siguen faltando **datos de paper** (y las cuentas para votar, decisión 9).

## Fase 9: Observabilidad y operación segura

- **Criterio:** el ROADMAP **no escribe uno**. Pide logs estructurados, alertas (Telegram o email), y arranque y parada seguros sin órdenes huérfanas.
- **Mergeado:** logs JSON sin secretos y alertas (#300); arranque y parada seguros (#302); estado de la operación y del riesgo en la UI (#317).
- **Evidencia:** tests de `cfds-obs` y del gateway (`ops`, arranque y parada). Telegram es opcional y está apagado si no hay token.
- **Verificar:** `cd engine && cargo test -p cfds-obs -p cfds-gateway`; en la UI, la barra de estado.
- **Aprobación:** registrada en #352 (24-11-2026), fuera de orden, con este alcance como criterio.
- **Estado:** **aprobada**. Prender Telegram sigue a decisión del dueño.

## Fase 10: UI y app del celular

- **Criterio:** el ROADMAP **no escribe uno**. Pide la app privada para los 6 iPhone, con push, aprobación de propuestas, votaciones, cuentas con 2FA y P&L, gráficos, noticias, parámetros, la vista del fondo y el diccionario.
- **Mergeado, entre otros:** prototipo (#31, #165 y #172); liquidez (#233 y #244); pestañas (#289); terminal paper (#276 y #293); PWA instalable (#309); fondo (#313); estado (#317); vistas de las fases 8 y 8b (#335); gráficos navegables (#404); aviso «sin velas en vivo» en el ticket (#386); transición de entrada (#416); revisión de calidad de todas las vistas (#423); gestos táctiles de los gráficos, con el scroll de la página libre (#425); la tarjeta «Modelo (informativo)» con su calibración (#434 y #442).
- **Evidencia:** la revisión #423 recorrió todas las vistas a 1400, 1024, 768 y 375 px, en tema claro y oscuro: 0 errores de consola, ninguna vista con scroll horizontal y ningún texto que prometa ganancias.
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

1. **Tasa de financiación de US500, US100 y US30** (`funding_benchmark` pendiente en `config/instruments.toml`: ¿es €STR, como en DE40?). Confirmarlo con IG desbloquea sus backtests y la fase 5 en EE. UU. En el mismo pedido a IG: confirmar que **IG España ajusta los dividendos** en los CFD de índices (`mode` de `[futures.basis]` en `config/candle_sources.toml`, #470).
2. **DE40: separación de datos o más historia.** `data_split.toml` pide 39 meses y hay 19 de futuros. Las salidas son más historia de FDAX o una separación distinta para la evaluación (también afecta a ML v1, §5).
3. **Cambios de estrategia** después del diagnóstico (#324): horario de apertura, distancia del stop, TP1 o break-even, peso de "patrón de velas + rechazo en nivel". Se evalúan con walk-forward antes de tocar la config.
4. **Acceso desde la red local o el celular:** el gateway escucha en `127.0.0.1:8787` (`config/gateway.toml`). Abrirlo pide decidir cómo, por ejemplo LAN con el token o una VPN.
5. **Telegram:** las alertas están listas (`config/alerts.toml`); falta decidir si se activan y crear el bot (`TELEGRAM_BOT_TOKEN` y `TELEGRAM_CHAT_ID`).
6. **Cuentas para votar:** quién vota y cómo se identifica cada uno (spec §1.2). Hace falta para las votaciones de la fase 8b y para las cuentas con 2FA de la fase 10.

## Decisiones tomadas el 24-11-2026

- **Runner de paper:** el dueño pidió prenderlo ("prendé el runner paper", #380). El PR #382 queda en borrador hasta que vuelvan las velas en vivo de IG (después del 27-11-2026 a las 20:30 UTC).
- **`CFDS_GATEWAY_TOKEN`:** el dueño lo definió en el `.env` del Mac mini.
- **Crontab:** el dueño lo instaló con las rutas de `~/Documents/CFDsTrader Home`.
- **Capital de paper:** 10 000 € simulados ("dale, poné 10.000 € de capital para el paper", #484 y #485).
- **ML después de v1 y v2:** se cierra por ahora; se sigue mostrando B1 (#158). Textual: "dale, vos hace lo que sea necesario".
- **Investigación:** se corrió la ronda 2 con la tendencia mensual (A) y el momentum intradía (B) (#486 y #490). Ninguna pasa, y el presupuesto de error de #466 quedó agotado. El dueño pidió un **programa nuevo** que cambie la pregunta (#493), sumando los patrones de manipulación detectables (#492 y #494).
