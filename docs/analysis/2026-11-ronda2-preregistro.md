# Pre-registro de la ronda 2: tendencia mensual (A) y momentum intradía (B)

Issue #486 · 24-11-2026 · base: `docs/analysis/2026-11-propuesta-investigacion.md` (#466, §3.A, §3.B y §4) · método: el de la ronda 1 (`docs/analysis/2026-11-ronda1-preregistro.md`, #472).

Este archivo se commitea **antes de correr nada contra la base**. La hora del commit que lo agrega es la del pre-registro. Todo lo que #466 fija se sigue al pie de la letra; acá se fija **solo lo que #466 deja abierto**, para que no quede nada por elegir después de ver un resultado. Cambiar algo de acá después de correr invalida la familia y la cuenta como una prueba nueva (#466 §4.1).

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro.

## 1. Lo que viene de #466 y de la ronda 1 sin cambios

- Familias A y B con sus definiciones de #466 §3.A y §3.B. La regla de decisión de #466 §4, **sin cambios**.
- Walk-forward de `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3). El resultado es la **concatenación de las ventanas de validación** (esperado: 02-2013 → 10-2025, 51 ventanas, 153 meses).
- **El fuera de muestra (desde el 01-11-2025) no se carga, no se mira y no se usa.** Las velas se piden solo hasta el 01-11-2025 00:00 UTC; para planificar la separación se lee solo la hora de la última vela guardada.
- Velas de 15m de los futuros de Databento (ES, NQ, YM), `--fuente futuros`, con la preparación común (`examples/common/prepare.rs`: validación §7.8 y §12, horario §2 y §7.1, vencimientos con cierre forzado, base del futuro de #470). Sin noticias (no hay calendario guardado para el período). Conexión a la base con `default_transaction_read_only=on`.
- Costos: spread típico y slippage de config (`fill_price`), comisión, financiación de IG **noche por noche** con los dos escenarios empalmados de #421 (EUR: EONIA→€STR, `docs/analysis/2026-11-eur-eonia-estr.csv`; USD: EFFR→SOFR, `docs/analysis/2026-11-usd-effr-sofr.csv`), **menos la base del futuro** (#470, `[futures.basis]` de `config/candle_sources.toml`), y el estrés de costos × 1,5. Las mismas funciones de costos que el backtest de la estrategia en uso.
- Cuenta y riesgo: igual que la ronda 1 (pre-registro de la ronda 1 §2.2). **Cada índice con su propia cuenta de 100 000 €**, nueva en cada ventana de validación; todas las operaciones pasan por `cfds_risk::check` con `config/risk.toml` sin cambios (1 % por operación, pérdida diaria 3 %, drawdown 15 % con el kill switch que en backtest se reactiva a los 20 días hábiles de `config/backtest.toml`, stop entre 0,5 y 3 ATR, apalancamiento máximo 10:1 y margen del catálogo). Entrada de referencia para el riesgo = cierre de la vela de la decisión; ejecución en la apertura de la vela siguiente.
  - **Por qué 100 000 €** (también para A): con 10 000 € y 1 % de riesgo, el tamaño mínimo de IG no deja abrir A en ningún índice con un stop de 3 ATR diarios (#466 §3.A: harían falta ~17 000 € en US500, ~39 000 € en US100 y ~25 000 € en US30). Con 10 000 € se probaría el tamaño mínimo y no la hipótesis. En el backtest el tamaño es continuo, así que con 1 % de riesgo el resultado en % no depende del capital salvo por el mínimo. Es el mismo capital de la ronda 1, para que las rondas sean comparables. El capital real lo decide el dueño (#466 §6.3); el informe cuenta aparte cuántas operaciones **no** se habrían podido abrir con 10 000 €.
- Ejecución de cada pata (función `run_plans` de la ronda 1): una posición a la vez; en cada vela primero se mira si la apertura ya saltó el stop (sale a la apertura), después si el mínimo o el máximo lo tocó (sale al stop), después la salida planificada y después un cierre forzado de la preparación (fin de semana o vencimiento: sale al cierre de esa vela). Si salta el stop, el resto del evento no se abre.
- Retornos mensuales, cartera y estadística como en la ronda 1 §2.5 y §3.1: retorno de una operación = P/L neto sobre el capital realizado de su cuenta al decidir; el mes de un índice compone las operaciones que **se cierran** en él (UTC), sin operaciones 0; la cartera es el promedio simple de los retornos mensuales de US500, US100 y US30. Las estadísticas en `f64` (no son dinero); todo lo que es dinero en `Decimal`.

## 2. Lo que se fija acá

### 2.1 Presupuesto de error y unidad de prueba

- La ronda 1 gastó 2,5 % (C y D1, 1,25 % cada una). Entran en la ronda 2 **dos** familias (A y B): el 2,5 % restante dividido por 2 = **1,25 % unilateral por familia** (#466 §4.2). Con esto el presupuesto de 5 % del programa queda **agotado**: cualquier familia o variante posterior queda fuera del presupuesto pre-registrado y necesita una decisión nueva del dueño.
- Una hipótesis por familia: la cartera con igual riesgo en US500, US100 y US30. US2000 se informa aparte, **descriptivo y sin voto**, con los costos supuestos «proporcionales» de #421 (`docs/analysis/2026-11-us2000-supuestos-proporcional.toml`), validación desde 08-2019 si los datos de RTY están (desde 2017).

### 2.2 Familia A: tendencia mensual (time-series momentum)

Definición de #466 §3.A (Moskowitz, Ooi y Pedersen, 2012). Detalle operativo:

- **Revisión mensual.** Para cada mes con sesión en XNYS (`config/sessions.toml` y sus feriados, conocidos de antemano), la revisión es la **primera decisión permitida** desde el cierre de la anteúltima vela de 15m de la sesión del **último día hábil del mes** (la que termina 15 minutos antes del cierre). Es el mismo punto de decisión que la D1 de la ronda 1. "Al cierre de la sesión" de #466 se toma así porque la ventana de entradas de la spec (§2, `config/trading_window.toml`) no deja decidir en el cierre mismo de la sesión; la decisión más cercana permitida es la de 15 minutos antes, con entrada en la apertura de la última vela.
  - "Decisión permitida" = la marca de entrada de la preparación está en verdadero (horario, calidad de datos, vencimientos) y la vela siguiente empieza justo al cierre de la de la decisión (sin hueco de datos).
  - Si el último día hábil es el último de su semana (viernes, o jueves con el viernes feriado), la spec no deja abrir en sus últimos 30 minutos: la revisión pasa a la primera decisión permitida del día hábil siguiente (en general, el lunes al cierre de la primera vela de 15m de la sesión, entrada a las 09:45 ET). Lo mismo si la decisión está bloqueada por otro motivo.
- **Señal.** Retorno de 12 meses = retorno desde el cierre de la vela de la revisión de **12 meses antes** hasta el cierre de la vela de la revisión actual, **dentro de cada contrato y encadenado en los vencimientos**: en cada cambio de contrato (los instantes de vencimiento de la preparación, #269) el retorno se corta en el cierre de la última vela del contrato viejo y sigue desde la apertura de la primera vela del contrato nuevo, así que el salto de la base del vencimiento no entra en la señal. Retorno > 0: largo; < 0: corto; = 0: sin posición ese mes.
  - **Sin 12 meses de historia** (no existe la revisión de 12 meses antes, por ejemplo el primer año de datos o un mes sin revisión por datos faltantes): sin posición ese mes.
  - La señal queda fija hasta la revisión siguiente.
- **Patas.** Desde la revisión del mes hasta la revisión siguiente, la posición se abre en la apertura de la vela siguiente a la decisión. Un cierre forzado (fin de semana, spec §8, o vencimiento) la cierra; se reabre en la **primera decisión permitida** siguiente dentro del mes, en la misma dirección, como una operación nueva con su propio paso por el riesgo y su stop recalculado. Así se cumple "cierra el viernes y reabre el lunes en la apertura si la señal sigue".
- **En cada revisión la posición se cierra y se abre de nuevo**, aunque la señal no cambie de signo: la pata del mes sale al cierre de la vela de la revisión siguiente y la del mes nuevo entra en la apertura de la vela siguiente. Cuesta un spread más por mes (≈ 0,006 R en US500, #466 §3.A) y rehace el tamaño (1 %) y el stop con el ATR vigente. Es la opción más simple y no favorece a la familia.
  - Si no existe la revisión siguiente (la de octubre de 2025 cae en el fuera de muestra porque el 31-10-2025 es viernes), la pata termina en el cierre forzado del viernes 31-10-2025 o en la última vela cargada.
- **Stop de protección: 3 × ATR(20) diario, medido en ATR diario.** Es parte de la definición de la familia (#466 §3.A), no el ATR de 15m de la estrategia en uso. El ATR es el de Wilder (`cfds_indicators::wilder_atr`, con rango verdadero) sobre diarias armadas con las velas de 15m de la sesión de contado (las mismas `session_bars` de la D1), usando solo las diarias de los días **anteriores** a la decisión. Para el riesgo, el ATR con el que se mide la distancia al stop es **ese mismo ATR diario**, así que el stop queda siempre a 3 ATR, dentro del tope de `stop_distance.max_atr` (3, inclusive) **sin cambiar `config/risk.toml`**. Largo: stop = referencia − 3 × ATR; corto: referencia + 3 × ATR.
- **Si salta el stop**, no se reabre hasta la revisión del mes siguiente.
- **Criterio 5 (contra comprar y mantener)**, solo para A:
  - Comprar y mantener el CFD = en cada ventana de validación y en cada índice, un largo de 1 × el capital (tamaño = capital / precio de entrada, en la moneda del instrumento) que entra en la apertura de la primera vela de la sesión de contado de la ventana y sale al cierre de la última vela de la sesión de contado de la ventana. **Se mantiene los fines de semana** (el CFD no vence ni cierra) y en los vencimientos del futuro se encadena sin costo (el CFD no rola): el P/L de precio se corta en el cierre de la última vela del contrato viejo y sigue desde la apertura de la primera del nuevo.
  - Costos: spread y slippage de config en la entrada y la salida de cada ventana (cada 3 meses: más costo que el CFD real, que se compra una sola vez; perjudica a comprar y mantener, no a A), financiación de IG en cada corte de cada noche sobre el cierre de la última vela anterior al corte, menos la base del futuro, con el mismo escenario.
  - Retorno mensual (UTC): valor a fin de mes (capital + P/L de precio a la última vela cerrada antes del fin de mes − costos acumulados) sobre el valor a fin del mes anterior (o el capital inicial de la ventana). Cartera = promedio de los tres índices.
  - Pasa si el Sharpe mensual de la cartera de A ≥ el Sharpe mensual de la cartera de comprar y mantener, en los 153 meses, **con los dos escenarios**.

### 2.3 Familia B: momentum intradía (primera media hora → última media hora)

Definición de #466 §3.B (Gao, Han, Li y Zhou, 2018). Horarios en **America/New_York** (la zona de la sesión de XNYS: el cambio de horario se resuelve solo). Detalle operativo, para cada día hábil `d` de XNYS:

- **Señal:** signo del retorno desde el cierre de la última vela de 15m de la sesión del **día hábil anterior** (la que termina en el cierre de esa sesión: 16:00 ET, o la hora del cierre anticipado) hasta el cierre de la vela de 15m que termina **30 minutos después de la apertura** de `d` (10:00 ET). Encadenado en los vencimientos como en A. Retorno > 0: largo; < 0: corto; = 0: no se opera. Si falta alguna de las dos velas, no se opera.
  - Esa ventana incluye la noche: se usa el precio del futuro fuera del horario del contado (#466 §3.B).
- **Decisión** al cierre de la vela que termina **30 minutos antes del cierre** de la sesión de `d` (15:30 ET; en un día de cierre anticipado, 30 minutos antes de ese cierre); **entrada** en la apertura de la vela siguiente; **salida** al cierre de la vela que termina en el cierre de la sesión (16:00 ET). Si falta la vela de la decisión, la de la entrada (hueco) o la de la salida, ese día no se opera. Tiene que ser una decisión permitida (marca de entrada de la preparación).
- **Sin viernes:** el último día hábil de la semana (viernes, o jueves con el viernes feriado) no se opera: la spec §2 no deja abrir en los últimos 30 minutos de ese día, y la marca de entrada de la preparación ya lo bloquea.
- **Stop de protección: 2 × ATR(14) de 15m** en la vela de la decisión (el ATR de Wilder de 15m con el período de la estrategia en uso, el mismo que usó la C de la ronda 1). Para el riesgo, el ATR es ese mismo, así que el stop queda a 2 ATR.
- Una operación por día y por índice. Cierra en el día: sin financiación (el corte de las 23:00 de Madrid es posterior a las 16:00 ET todo el año).

### 2.4 Criterios (#466 §4.3, con los valores de esta ronda)

- **Criterio 1:** prueba t de Student de una muestra, unilateral (H0: media ≤ 0), sobre los 153 retornos mensuales de la cartera. Pasa si p < **0,0125** con **los dos** escenarios de financiación.
- **Criterio 2:** Sharpe deflactado con la fórmula de la ronda 1 §2.5 (Sharpe mensual sin anualizar, V = 1/(T − 1), asimetría y curtosis de la muestra), con **N = 352**: las 350 de la ronda 1 (que ya incluían C y D1) más A y B. Pasa si DSR ≥ 0,95 con los dos escenarios. Se informa también, solo como referencia, con N = 100 y N = 1000.
- **Criterio 3:** media mensual de la cartera > 0 en 02-2013 → 12-2019 **y** en 01-2020 → 10-2025, **y** media > 0 en al menos 2 de los 3 índices. Con los dos escenarios.
- **Criterio 4:** con costos × 1,5 (spread, slippage, comisión y financiación neta de la base, todos juntos, sobre las mismas operaciones y tamaños), la media mensual de la cartera sigue > 0. Con los dos escenarios.
- **Criterio 5 (solo A):** ver §2.2.
- A **pasa** solo si cumple los criterios 1 a 5; B, los criterios 1 a 4. **No concluyente** solo si la corrida no pudo hacerse.
- Según #466 §4.4: si una familia pasa, se presenta al dueño y **no** se toca el fuera de muestra; él decide. Si no pasa, se cierra y no se prueban variantes sobre estos datos.

### 2.5 Descriptivo, sin voto

Por familia e índice: operaciones, largos y cortos, salidas por motivo, rechazos del riesgo por motivo, P/L en R antes y después de costos, costo medio en R, aciertos, retorno mensual medio con y sin estrés, y operaciones que no se habrían abierto con 10 000 €. Para A, además, meses en largo, en corto y sin posición. US2000 como en §2.1.

### 2.6 Parámetros en config

Los valores de esta ronda (12 meses, ATR(20) diario, stop a 3 ATR; señal hasta 30 minutos después de la apertura, decisión 30 minutos antes del cierre, ATR(14) de 15m, stop a 2 ATR; nivel 0,0125, N = 352, DSR ≥ 0,95, estrés × 1,5, segunda mitad desde 2020-01-01) quedan en `config/research.toml`, validado al cargar, en secciones nuevas. La sección `[rule]` de la ronda 1 no cambia (así la ronda 1 se puede repetir igual). Solo los lee el ejemplo nuevo `ronda2`. `config/strategy.toml`, `config/risk.toml`, el runner y el paper no cambian.

### 2.7 Registro de corridas

Cada corrida (aunque falle o se repita por un error de código) se anota en el informe `docs/analysis/2026-11-ronda2-tendencia-y-momentum-intradia.md` con su hora y su comando. Si una corrida se repite por un error de código, se dice cuál fue el error y que la regla no cambió.
