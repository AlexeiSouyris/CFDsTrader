# Laboratorio de estrategias (diseño)

> **Estado: propuesta (#710, eje 3 de la épica #707).** Solo documentación: no hay código, config
> ni contratos en este PR. Los contratos se nombran pero no se escriben (contratos primero, en su
> propio PR). Cada decisión abierta está en **«Puntos para el dueño» (§9)** como **PENDIENTE**.
> Nada se implementa hasta que el dueño apruebe el diseño en el Issue o en el PR.
>
> Hecho por un agente de IA (Claude Code). Nada de esto es una recomendación de inversión.

Pedido del dueño (26-11-2026): «algunos traders hacen backtests y calculos de riesgo en estas
terminal [...] estamos haciendo lo mismo pero vamos al mas alla».

## Resumen simple

1. **Qué es.** Una vista donde el dueño arma una estrategia con **bloques** (entrada, stop, salida,
   filtros, tamaño), la corre sobre datos históricos guardados y recibe un **informe honesto**.
2. **Quién calcula.** La UI solo **arma una descripción** de la estrategia (una "spec" en JSON). El
   **motor en Rust** la valida y la corre con lo que ya existe: indicadores, patrones, liquidez,
   riesgo, costos del catálogo y el arnés de investigación (`research::*`). La UI nunca calcula.
3. **Qué tiene de "más allá".** Las terminales comunes muestran una curva linda. Acá cada informe
   trae además lo que la curva esconde: cuántas variantes se probaron (Sharpe deflactado y PBO),
   la mala suerte posible (Monte Carlo y riesgo de ruina), si le gana a **entrar al azar**, si
   aguanta **costos × 1,5**, las dos mitades, el walk-forward y comprar y mantener.
4. **Dos modos.** **Exploratorio**: libre, pero cada resultado dice "exploratorio, no vota" y queda
   contado. **Confirmatorio**: solo con un pre-registro congelado (hash) que paga su alfa del
   presupuesto global aprobado (#660). El fuera de muestra **nunca** se toca desde el laboratorio
   salvo la única evaluación de una confirmatoria que pasó.
5. **Calculadora y simulador.** Una calculadora de riesgo en vivo (tamaño, margen, pérdida si salta
   el stop) y un simulador de varianza (miles de caminos de capital a partir del win rate, el R medio
   y el riesgo por operación: rachas, caídas, ruina y probabilidad de pasar una prop firm).
6. **Corridas largas** en segundo plano, con progreso en vivo (SSE) y resultados de solo agregar,
   atados al hash de la spec: la misma spec con los mismos datos da siempre el mismo informe.
7. **Plan.** 13 PRs chicos (§7). Los primeros dan valor sin UI: un ejemplo de línea de comandos que
   corre una spec desde un archivo.

### Relación con la Academia (#709)

`docs/design/2026-11-academia.md` (en curso, otro agente) define el temario, **qué herramientas se
desbloquean y cuándo**, los exámenes con replay y el **simulador de cuentas fondeadas**. Este
documento no repite nada de eso:

- **Desbloqueos:** el laboratorio expone sus herramientas por partes (calculadora, simulador,
  constructor, modo confirmatorio) para que la Academia decida cuándo se abren. Las reglas de
  desbloqueo viven en la Academia.
- **Prop firms:** las **reglas** de cada cuenta fondeada (objetivo, pérdida diaria, pérdida total,
  días mínimos, plazo) y su contrato los define la Academia. El **cálculo** de los caminos de
  capital es uno solo: el simulador de varianza de este documento (§4.2), que la Academia usa con
  esas reglas. Un cálculo, un lugar (`CLAUDE.md` §1).
- Antes del PR de contratos (L1, §7) los dos documentos tienen que coincidir en los nombres.

## 0. Qué ya existe y se reutiliza

| Necesidad | Ya existe (no se reescribe) |
|---|---|
| Ejecutar planes de operación con riesgo y costos | `cfds_backtest::research::{Plan, execute_plan, run_plans}` (`research/run.rs`, `research/mod.rs`): decisión al cierre de `t`, ejecución en `t+1`, stop, objetivo en R, cierre forzado |
| Costos | `InstrumentCosts::from_spec` (bloquea si falta alguno), `FundingRates`, `FuturesBasis`, `cost_stress` = 1,5 |
| Tamaño y límites | `cfds-risk` `check` con `config/risk.toml` (grupos, gaps, caída gradual, kill switch) |
| Estadística | `research::stats`: prueba t, Newey-West, Welch, Sharpe deflactado, alfa HC1 |
| Monte Carlo | `research::montecarlo` (#662): bootstrap por bloques, caída máxima, racha, tiempo bajo el agua, ruina |
| Sobreajuste | `research::pbo::pbo_cscv` (#664), matriz meses × variantes |
| Test de suerte | `research::program4::LuckTest` (entradas al azar con SplitMix64), hoy atado al programa 4 |
| Walk-forward y fuera de muestra | `evaluation` (#298) con `config/data_split.toml` y el guardia `research_period`; `variants` (#359); `sensitivity` |
| Comprar y mantener | `buy_and_hold_returns` y la curva con costos y financiación |
| Cortes descriptivos | ejemplo `diagnostico` (#323, #456) |
| Señales | `cfds-indicators`, `cfds-patterns`, `cfds-liquidity`, `cfds-regime`, filtro de noticias §7.2 |
| Criterios de veredicto | `examples/common/alpha_verdict.rs` (los cinco criterios del alfa) |
| Presupuesto y N | `[alpha_budget]` de `config/research.toml`, libro "Presupuesto global" de `registro-de-resultados.md`, `tools/research/check_dsr_trials.py` |
| Corridas guardadas | tablas de solo agregar del `journal` (`backtest_runs`, `backtest_run_curves`) |
| SSE en el gateway | `GET /api/v1/ibkr/live` (`ibkr_live`, #558) |
| Token local anti-CSRF | rutas del broker simulado (`[paper] token_env`) |

Lo nuevo es poco: **traducir una spec a `Plan`s**, un **informe** que junta lo que ya existe, el
**simulador de varianza**, el **corredor de trabajos** y la UI.

## 1. Constructor de estrategias

### 1.1 Bloques

La UI muestra un formulario por bloques. **Qué bloques existen y qué parámetros aceptan lo dice el
motor** (`GET /api/v1/lab/catalog`, contrato `strategy_catalog.v1`): la UI dibuja el formulario a
partir de esa lista, sin nada escrito a mano.

| Bloque | Qué elige el usuario | De dónde sale en el motor |
|---|---|---|
| Mercado | instrumento, timeframe (15m, 1h, 1d), fuente de velas (IG, futuros, IBKR) | catálogo de `config/instruments.toml`, `candle_sources` |
| Entrada | 1 a 3 condiciones unidas con "y", más la dirección | señales ya calculadas por el motor (ver 1.2) |
| Filtros | horas UTC de entrada, régimen (`regime`), régimen del VIX, ventana de noticias de alto impacto (minutos antes y después), excluir días de vencimiento | `regime`, filtro §7.2, `roll_instants` |
| Stop | k × ATR, estructural (último pivote confirmado) o el stop de la señal | `indicators`, `patterns` (pivotes confirmados) |
| Salida | objetivo en R, salida por tiempo (N velas o cierre de sesión), trailing en ATR | `Plan::target_r`, `exit_index`, salidas de §8 |
| Tamaño | riesgo % por operación, **nunca más** que el de `config/risk.toml` | `cfds-risk` `check` (sin atajos, `CLAUDE.md` §5) |
| Costos | los del catálogo; el usuario solo puede **subirlos** (estrés ≥ 1) | `InstrumentCosts::from_spec` |

### 1.2 Catálogo inicial de señales (propuesta, PENDIENTE)

Pocas al principio, todas ya calculadas por el motor con velas cerradas:

1. Cruce de dos medias (SMA/EMA) en el timeframe elegido.
2. RSI que cruza un umbral.
3. Primer toque de un FVG (`cfds_liquidity::fvg_events`, como el programa 5).
4. Sweep de un pivote de 1h que cierra adentro (`liquidity`).
5. Ruptura del rango de apertura (`orb_plans`, ronda 1).
6. Patrón de vela confirmado (martillo, envolvente) sobre un nivel (`rejection_state_at`).

Más señales entran de a una, cada una en su PR con tests de no repintado.

### 1.3 La spec

- La UI arma un JSON `strategy_spec.v1` (solo el nombre por ahora). Lleva: versión del catálogo,
  mercado, bloques con sus parámetros, período pedido, modo (exploratorio o confirmatorio) y, si es
  confirmatoria, la referencia al pre-registro.
- **Hash de la spec:** SHA-256 de su forma canónica (claves ordenadas, decimales como texto, sin
  espacios). Identifica la estrategia en todo el sistema.
- **Sin campos inventados:** todo campo existe en el contrato; el motor rechaza los desconocidos.
- **Límites de complejidad** (contra el sobreajuste): como mucho 3 condiciones de entrada y 2
  filtros por spec en la v1 (PENDIENTE, §9). Nada de "O" en la v1.

### 1.4 Qué hace el motor

Un módulo nuevo `cfds_backtest::lab` (fila nueva en el registro de módulos cuando se cree):

- `validate_spec(spec, catalog, config) -> Result<ValidSpec, Vec<SpecError>>`: bloque desconocido,
  parámetro fuera de rango, falta el stop, riesgo mayor que el de config, costo menor que el del
  catálogo, período que toca el fuera de muestra (salvo §2.4), timeframe sin datos. Los errores
  van a la UI con `spec_validation.v1`.
- `plans_from_spec(spec, series) -> Vec<Plan>`: **función pura** que traduce la spec a los `Plan`
  que ya ejecuta `run_plans`. La señal se decide al **cierre** de `t` y se ejecuta en `t+1`; el
  timeframe mayor solo con velas cerradas; los pivotes solo confirmados; calentamiento de cada
  indicador respetado (`CLAUDE.md` §5b).
- Tests: no repintado y no mirar el futuro como propiedades (`proptest`, #663): cortar la serie en
  cualquier vela no cambia los planes anteriores.
- La v1 corre **un instrumento por cuenta** (como `run_plans`). Cartera de varios queda para después.

## 2. Barandas

### 2.1 Dos modos

| | Exploratorio | Confirmatorio |
|---|---|---|
| Para qué | probar ideas, aprender | votar una hipótesis |
| Requisito | ninguno | pre-registro congelado, con hash, en el libro de alfa |
| Datos | período de investigación (antes del fuera de muestra) | el mismo, con el walk-forward del pre-registro |
| Etiqueta del informe | **"exploratorio, no vota"** en todas las cifras | el criterio pre-registrado da el veredicto |
| Alfa | no gasta | paga `α_j/(1 − α_j)` **al congelarse**, aunque no se corra |
| Fuera de muestra | nunca | una sola vez, solo si pasó (§2.4) |

### 2.2 Contar la búsqueda

La regla aprobada (#660, §4.1) dice que **lo que no vota no suma al N del proyecto**. El laboratorio
la respeta y además no esconde la búsqueda:

- Cada corrida exploratoria deja una fila de solo agregar (`lab_trial.v1`: hash de la spec,
  familia, fecha). Una spec repetida no suma de nuevo.
- **Familia** = mismo instrumento y mismos tipos de bloque; cambian solo los parámetros
  (definición PENDIENTE, §9).
- El **Sharpe deflactado** que muestra el laboratorio usa `N = N del proyecto (dsr_trials vigente)
  + variantes distintas de la familia`. Nunca menos que el N del proyecto.
- El **PBO** se calcula sobre todas las variantes de la familia (`pbo_cscv`), desde 4 variantes.
- Un contador visible: "probaste 23 variantes de esta familia".
- **Al congelar** una confirmatoria, el pre-registro declara cuántas exploratorias de su familia se
  hicieron. Propuesta: que esas sumen al N del proyecto (cambia §4.1 de la regla y el script del
  CI; PENDIENTE, §9).

### 2.3 Congelar una confirmatoria

- El laboratorio arma un **borrador del pre-registro** con la plantilla de siempre: hipótesis, spec
  y su hash, criterio de voto (los cinco criterios del alfa de `alpha_verdict.rs`), `W` antes y
  después, `α_j`, filtro de borde bruto con sus números y N al congelar.
- **Dónde vive el libro:** sigue siendo la sección "Presupuesto global" de
  `registro-de-resultados.md`, verificada por el CI (punto 16 aprobado). No se crea un segundo
  libro en la base de datos: nunca dos versiones de lo mismo.
- Por eso, en la v1 **congelar es un PR** (como hoy): el borrador + su `.sha256` + la fila del
  libro. Lo mergea el dueño o su agente con el CI en verde.
- El motor corre una confirmatoria solo si recibe un pre-registro cuyo SHA-256 coincide **y** cuya
  fila está en el libro de `main`. Si no, la rechaza.

### 2.4 Fuera de muestra bloqueado

- El guardia `research_period` de `evaluation` rechaza cualquier corrida del laboratorio que toque
  los últimos 12 meses (`config/data_split.toml`). En modo exploratorio el botón no existe.
- Una confirmatoria que pasó puede evaluarse en el fuera de muestra **una sola vez** por hash: el
  resultado queda guardado y un segundo pedido se rechaza. Si confirma, se registra el premio `ω`.

### 2.5 Costos nunca en cero

- Costos del catálogo con spread, slippage, comisión y financiación nocturna (y la base del futuro
  cuando la fuente son futuros). Si falta alguno, la corrida no arranca (`from_spec` ya bloquea).
- El usuario solo puede **subirlos** (multiplicador ≥ 1). Todo informe trae además la versión con
  costos × 1,5.

### 2.6 Filtro de borde bruto a la vista

- El motor calcula el **costo por operación `C`** de la spec con la fórmula aprobada (§3.2 de la
  regla): spread de la franja de entrada y salida, slippage, comisión y noches esperadas en largo.
- El informe muestra `C`, la barra `k × C` (k = 2, `edge_cost_factor`) y el borde bruto medido. En
  exploratorio, el bruto medido es un **resultado**, y se dice: "esto no vale como estimación
  previa".
- En confirmatorio el pre-registro trae la **estimación previa** (literatura con cita, o `IC × σ_h`
  con `σ_h` descriptivo que calcula el motor sin mirar la regla). Si no cumple, no se congela.

## 3. Informe

Todo lo calcula el motor y llega a la UI en `lab_report.v1`. Cada bloque dice si es
**descriptivo** o si **vota** (solo el criterio pre-registrado de una confirmatoria vota).

| Bloque | Qué muestra | De dónde sale |
|---|---|---|
| Curva y caída | capital diario, caída desde el máximo, comprar y mantener superpuesto | recorrido de `run_plans`, `buy_and_hold_returns` |
| Distribución de R | histograma del R neto por operación, R bruto al lado | `StudyTrade`, `trade_return` |
| Esperanza | R medio neto con IC; win rate con IC de Wilson; profit factor | `stats`, `indicators::wilson` |
| Sharpe | Sharpe, Sortino y **Sharpe deflactado** con su N | `stats::deflated_sharpe` |
| PBO | probabilidad de sobreajuste de la familia y los logits | `pbo_cscv` |
| Monte Carlo | bandas p5/p50/p95 de capital, caída máxima, racha perdedora, tiempo bajo el agua, **riesgo de ruina** | `research::montecarlo` |
| Walk-forward | resultado por ventana, % de ventanas positivas | `evaluation` |
| Mitades | primera contra segunda mitad | nuevo, trivial sobre los retornos |
| Costos × 1,5 | todo lo anterior con estrés | `cost_stress` |
| Test de suerte | percentil de la estrategia entre N entradas al azar con **las mismas salidas y el mismo tamaño** (modelo nulo de Basso) | `LuckTest`, generalizado (L5) |
| Contra comprar y mantener | retorno, Sharpe y caída; con costos y financiación | `buy_and_hold_returns` |
| MAE/MFE | cuánto llegó a perder y a ganar cada operación antes de cerrar, en R | **nuevo** en `StudyTrade` (L4, C6 del autoanálisis) |
| Cortes | por año, hora y dirección, con aviso "mirar cortes es probar más" | `diagnostico` |

- Lenguaje: probabilidades e incertidumbre, nunca promesas (`CLAUDE.md` §5). Ninguna cifra dice
  "ganancia esperada" sin su intervalo.
- Cada informe guarda: hash de la spec, hash de la config, versión del código (commit), última vela
  usada, semilla de las simulaciones. Con eso se reproduce exacto.

## 4. Calculadora de riesgo y simulador de varianza

### 4.1 Calculadora de riesgo en vivo

- Entradas: capital hipotético, riesgo %, instrumento, dirección, entrada y stop (precio o k × ATR).
- Salida: tamaño, margen, apalancamiento, pérdida si salta el stop (con el colchón por gaps
  `gap_atr`), costo de ida y vuelta, costo de una noche, y cada límite de `config/risk.toml` que
  recorta o rechaza.
- **Un solo cálculo:** el mismo `cfds-risk` `check` del broker simulado, sin estado de cuenta. Ruta
  propuesta `POST /api/v1/lab/risk_preview`; respuesta reutilizando `ticket_verdict.v1` si alcanza
  (se decide en L1).
- "En vivo": la UI pide de nuevo al cambiar un campo (con espera corta) y el precio de referencia es
  el cierre de la última vela cerrada. La UI no hace cuentas.

### 4.2 Simulador de varianza

- Entradas: win rate, R medio ganador, R medio perdedor (por defecto −1), riesgo % por operación,
  operaciones por camino, cantidad de caminos (tope en config, PENDIENTE), semilla.
- O bien **desde una corrida**: remuestrea los R reales de un informe en vez de los parámetros.
- **"Más allá":** opción de incertidumbre del win rate. Con N operaciones observadas, el win rate de
  cada camino se sortea de su distribución (Beta), no se toma como cierto. Muestra cuánto cambia la
  ruina cuando el 55 % "medido" puede ser 48 %.
- Salida: bandas de capital (p5, p25, p50, p75, p95), distribución de la caída máxima, probabilidad
  de una racha de k pérdidas seguidas, riesgo de ruina y, con un perfil de prop firm de la Academia,
  probabilidad de **pasar, quemar la cuenta o quedarse sin tiempo**.
- Motor: función pura con SplitMix64 (la de `montecarlo`), determinista con la semilla. Es
  estadística, no contabilidad: usa `f64` como `montecarlo`; los montos que se muestran como dinero
  salen en decimales.
- Ruta `POST /api/v1/lab/variance` (`variance_sim_request.v1` → `variance_sim_report.v1`).
  Sincrónica hasta un tope de caminos × operaciones; más grande va como trabajo (§5).

## 5. Corredor de trabajos

| Ruta (propuesta) | Qué hace |
|---|---|
| `POST /api/v1/lab/jobs` | valida la spec y encola; devuelve `job_id` (idempotente: mismo pedido, mismo id) |
| `GET /api/v1/lab/jobs/{id}/events` | **SSE** con `lab_job_event.v1`: en cola, corriendo (etapa y %), listo (`report_id`), falló (error), cancelado; reconecta con `Last-Event-ID` |
| `GET /api/v1/lab/jobs/{id}` | estado actual |
| `DELETE /api/v1/lab/jobs/{id}` | cancela |
| `GET /api/v1/lab/reports/{report_id}` | el informe guardado |
| `GET /api/v1/lab/trials?family=` | variantes probadas de una familia |

- **Dónde corre (PENDIENTE, §9):** recomendado un **proceso aparte** (`lab_worker`) y no dentro del
  gateway, para que un backtest largo nunca frene al runner de paper. El gateway solo encola, lee y
  retransmite el progreso.
- Cola y progreso: la cola en PostgreSQL (`lab_jobs`, eventos de solo agregar); el progreso en vivo
  por Redis Streams, que el gateway pasa a SSE (`CLAUDE.md` §1).
- Resultados en tablas **de solo agregar** (`lab_reports`, `lab_trials`) con el hash de la spec. Un
  informe nunca se edita; datos nuevos = informe nuevo.
- Límites en `config/lab.toml` (PENDIENTE): trabajos a la vez (1), duración máxima, variantes por
  trabajo, caminos del simulador.
- **Nunca llama a IG:** solo velas ya guardadas (cupo de IG, `docs/guides/trabajo-en-paralelo.md`).
- Seguridad: el mismo token local anti-CSRF de las rutas de paper, `application/json`, límite de
  body. El gateway deja de ser "solo lectura" en estas rutas: se actualiza su ficha.

## 6. Contratos a definir primero (solo nombres)

| Contrato | Para qué |
|---|---|
| `strategy_catalog.v1` | bloques y parámetros disponibles, con rangos |
| `strategy_spec.v1` | la estrategia declarativa que arma la UI |
| `spec_validation.v1` | errores de validación |
| `lab_job_request.v1` / `lab_job_event.v1` | pedido de trabajo y su progreso |
| `lab_report.v1` | el informe completo (§3) |
| `lab_trial.v1` | una variante probada (para el N y el PBO) |
| `variance_sim_request.v1` / `variance_sim_report.v1` | simulador de varianza |
| reglas de prop firm | **las define la Academia** (#709); el simulador las consume |
| vista previa de riesgo | reusar `ticket_verdict.v1` si alcanza; si no, `risk_preview.v1` |

## 7. Plan por fases (PRs chicos)

| # | PR | Depende de | Valor |
|---|---|---|---|
| L0 | este documento | — | diseño aprobado |
| L1 | contratos `strategy_catalog`, `strategy_spec`, `spec_validation` (alineados con la Academia) | L0 | contratos primero |
| L2 | `lab`: catálogo, `validate_spec` y hash canónico (puro, con tests) | L1 | |
| L3 | `plans_from_spec` con 2 señales (cruce de medias, ruptura del rango de apertura), stop en ATR, salida en R o tiempo; tests de no repintado | L2 | |
| L4 | MAE/MFE en `StudyTrade` (sin cambiar ningún resultado viejo: test de igualdad) | — | C6 del autoanálisis |
| L5 | test de suerte general `research::luck`, extraído del programa 4 con test de igualdad | — | modelo nulo estándar |
| L6 | `lab_report` + ejemplo `laboratorio` que corre una spec desde un archivo JSON | L3–L5 | **primer valor, sin UI** |
| L7 | simulador de varianza (función pura, contrato y ejemplo) | L1 | lo usa también la Academia |
| L8 | contratos de trabajos y `lab_trial`; migración `lab_jobs`, `lab_reports`, `lab_trials` | L6 | |
| L9 | `lab_worker` + rutas del gateway + SSE | L8 | corridas largas |
| L10 | rutas `risk_preview` y `variance` | L7 | |
| L11 | UI: calculadora y simulador | L10 | lo más simple primero |
| L12 | UI: constructor e informe | L9 | |
| L13 | flujo confirmatorio: borrador del pre-registro, chequeo del hash y del libro; ajuste del CI si el dueño aprueba §9 punto 1 | L6 | |

Señales nuevas (resto de 1.2) entran de a una después de L3. Cada PR, menos de 400 líneas sin tests.

## 8. Qué no hace

- No manda órdenes reales ni de paper. Pasar una estrategia del laboratorio al runner de paper es
  otro Issue.
- No tiene botón "optimizar" que busque la mejor combinación. Las variantes son explícitas (hasta un
  tope por trabajo) y cada una cuenta para el N y el PBO.
- No usa ML en la v1. No calcula nada en la UI ni en Python.
- No cambia la regla del presupuesto: si el dueño aprueba el punto 1 de §9, el cambio va en su
  propio PR.

## 9. Puntos para el dueño (todos PENDIENTES)

| # | Punto | Recomendación |
|---|---|---|
| 1 | ¿Las exploratorias de una familia suman al N del proyecto al congelar una confirmatoria de esa familia? | **Sí** (cambia §4.1 de la regla y el script, en otro PR). Siempre suman en el Sharpe deflactado del laboratorio |
| 2 | Definición de familia | mismo instrumento + mismos tipos de bloque |
| 3 | Límite de complejidad de la spec v1 | 3 condiciones de entrada y 2 filtros |
| 4 | Catálogo inicial de señales | las 6 de §1.2, empezando por 2 |
| 5 | Libro de alfa | sigue en `registro-de-resultados.md`; congelar = PR |
| 6 | Dónde corren los trabajos | proceso aparte (`lab_worker`) |
| 7 | Topes (`config/lab.toml`) | 1 trabajo a la vez; 20 variantes por trabajo; 10 000 caminos × 1 000 operaciones en el simulador |
| 8 | Reservar meses ciegos antes del fuera de muestra para confirmatorias del laboratorio | **No** en la v1: se mantiene la regla del fuera de muestra de 12 meses |
| 9 | Incertidumbre del win rate en el simulador | prendida por defecto |
| 10 | Orden de la UI | calculadora y simulador antes que el constructor |

## 10. Supuestos y riesgos

- **El laboratorio facilita el sobreajuste.** Es el riesgo principal. Las defensas son las de §2:
  conteo, Sharpe deflactado con N honesto, PBO, test de suerte, mitades, costos × 1,5, fuera de
  muestra bloqueado y etiqueta "no vota". Ninguna lo elimina.
- **CPU del Mac mini:** un trabajo largo compite con el runner de paper. Por eso el proceso aparte
  y un trabajo a la vez.
- **El gateway deja de ser solo lectura** en las rutas del laboratorio; hay que actualizar su ficha
  y proteger las rutas como las de paper.
- **Coordinación con la Academia:** su documento todavía no está en `main`; los nombres de §6 pueden
  cambiar hasta L1.
- **`f64` en las simulaciones:** igual que `montecarlo`; nunca para precios, P/L ni margen.
- **Datos:** el laboratorio solo ve lo guardado. Con spread fijo histórico (C5 del autoanálisis)
  los años viejos pagan de más; el informe lo dice.
