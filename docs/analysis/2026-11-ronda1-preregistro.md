# Pre-registro de la ronda 1: ruptura del rango de apertura (C) y cambio de mes (D1)

Issue #472 · 24-11-2026 · base: `docs/analysis/2026-11-propuesta-investigacion.md` (#466).

Este archivo se commitea **antes de correr nada contra la base**. La hora del commit que lo agrega es la del pre-registro. Todo lo que #466 ya fija se sigue al pie de la letra. Acá se fija **solo lo que #466 deja abierto**, para que no quede nada por elegir después de ver un resultado. Cambiar algo de acá después de correr invalida la familia, y la cuenta como una prueba nueva (#466 §4.1).

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro.

## 1. Lo que viene de #466 sin cambios

- Familias C y D1, con sus definiciones de #466 §3.
- Walk-forward de `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3). El resultado es la **concatenación de las ventanas de validación**.
- **El fuera de muestra (desde el 01-11-2025) no se carga, no se mira y no se usa.**
- Costos de config (spread típico, slippage de `config/backtest.toml`), financiación de IG con los **dos** escenarios empalmados de #421 (EUR: EONIA→€STR, `docs/analysis/2026-11-eur-eonia-estr.csv`; USD: EFFR→SOFR, `docs/analysis/2026-11-usd-effr-sofr.csv`), la corrección de la base de los futuros de #470 (`[futures.basis]` de `config/candle_sources.toml`) y el estrés de costos × 1,5.
- Una hipótesis por familia: la cartera con igual riesgo en US500, US100 y US30. Nivel **1,25 % unilateral** por familia (Bonferroni de la ronda 1).
- Criterios 1 a 4 de #466 §4.3 (el 5 es solo para A).

## 2. Lo que se fija acá

### 2.1 Datos y separación

- Velas de 15m de los futuros de Databento (ES, NQ, YM), `--fuente futuros`, con la preparación común de los estudios (`examples/common/prepare.rs`: validación §7.8 y §12, horario §2 y §7.1, vencimientos con cierre forzado y reinicio del calentamiento, base del futuro). Sin noticias (no hay calendario guardado para el período).
- Las velas se piden **solo hasta el inicio del fuera de muestra** (01-11-2025 00:00 UTC). Para planificar la separación se lee solo la hora de la última vela guardada (`last_candle_start_from`), no sus precios.
- Ventanas: las de `plan()` con esas fechas. Validación esperada: 02-2013 → 10-2025 (51 ventanas, 153 meses).
- Las dos familias no tienen parámetros que ajustar. Las señales se calculan una sola vez sobre toda la serie de investigación con funciones puras y sin mirar el futuro; en cada ventana de validación se ejecutan las operaciones **decididas** dentro de esa ventana, con una cuenta nueva. Es equivalente a recorrer cada ventana con su calentamiento: los 24 meses de entrenamiento previos alcanzan de sobra para los ATR.

### 2.2 Cuenta, riesgo y tamaño

- **Cada índice con su propia cuenta de 100 000 €** que arranca de cero en cada ventana de validación. La cartera con igual riesgo es el **promedio simple de los retornos mensuales de las tres cuentas**.
  - Por qué 100 000 € y no 10 000 €: con 10 000 € el tamaño mínimo de IG no deja abrir casi ninguna operación de D1 en US100 y US30 (stop de ~500 y ~850 puntos, #466 §2.3), y entonces no se probaría la hipótesis sino el tamaño mínimo. El tamaño en el backtest es continuo, así que con 1 % de riesgo el resultado en % no depende del capital salvo por el mínimo. Elegir el capital real es decisión del dueño (#466 §6.3). El informe cuenta, aparte, cuántas operaciones **no** se habrían podido abrir con 10 000 €.
- Todas las operaciones pasan por `cfds_risk::check` con `config/risk.toml` sin cambios: 1 % por operación, pérdida diaria 3 %, drawdown 15 % (kill switch, que en backtest se reactiva a los 20 días hábiles de `config/backtest.toml`), stop entre 0,5 y 3 ATR, tope de apalancamiento 10:1 y margen del catálogo.
  - **C:** el ATR del riesgo es el ATR(14) de 15m de la vela de la señal (el mismo que usa la estrategia actual). **Si el stop en el extremo opuesto del rango queda a más de 3 ATR (o a menos de 0,5), el riesgo rechaza la operación y ese día no se opera.** La familia no lleva un máximo propio (es la opción "más simple" de #466 §3.C y no cambia config).
  - **D1:** el ATR del riesgo es el mismo ATR(20) diario del stop, así que el stop queda siempre a 2 ATR.
- Entrada de referencia para el riesgo = cierre de la vela de la señal (como la estrategia actual); la ejecución es en la apertura de la vela siguiente, con medio spread y slippage.

### 2.3 Familia C: detalle operativo

- Sesión de contado de XNYS (`config/sessions.toml` y sus feriados; cierres anticipados incluidos). Rango = máximo y mínimo de las velas de 15m que empiezan en la apertura y 15 minutos después. Si falta alguna de las dos, ese día no se opera.
- Ruptura: la primera vela, desde la que empieza 30 minutos después de la apertura, cuyo **cierre** queda por encima del máximo (largo) o por debajo del mínimo (corto). La vela de la ruptura tiene que **cerrar a las 15:00 ET o antes**; si no, ese día no se opera. Un cierre igual al extremo no es ruptura.
- Entrada en la apertura de la vela siguiente, que tiene que empezar justo al cierre de la vela de la ruptura (si hay un hueco de datos, no se opera). La decisión tiene que estar permitida por las marcas de entrada de la preparación común (horario, calidad de datos, vencimientos).
- Stop en el extremo opuesto del rango. Salida al cierre de la última vela de 15m que termina en el cierre de la sesión (si falta, la última disponible de la sesión). Una sola operación por día.
- Dentro de cada vela: primero se mira si la apertura ya saltó el stop (sale a la apertura), después si el mínimo o el máximo lo tocó (sale al stop), y recién después la salida de la sesión. Un cierre forzado de la preparación (vencimiento o fin de semana) cierra al cierre de esa vela.

### 2.4 Familia D1: detalle operativo

- Último día hábil del mes y tercer día hábil del mes siguiente: días con sesión en XNYS según el calendario de feriados (conocido de antemano).
- Diarias para el ATR: una vela por día hábil armada con las velas de 15m de la sesión de contado (apertura de la primera, máximo, mínimo, cierre de la última). ATR(20) de Wilder (`cfds_indicators::wilder_atr`, con rango verdadero) sobre esas diarias. Una decisión del día D usa solo las diarias de los días **anteriores** a D (cerradas). El rango verdadero del día siguiente a un vencimiento incluye el salto de la base del contrato nuevo: se acepta (es chico frente al ATR) y se informa como límite.
- Entrada: decisión al cierre de la anteúltima vela de 15m de la sesión del último día hábil (la que termina 15 minutos antes del cierre) y entrada en la apertura de la última. Largo. Stop = entrada de referencia − 2 × ATR(20) diario.
- Salida al cierre de la sesión del tercer día hábil del mes nuevo.
- **Fin de semana:** el cierre forzado del último día hábil de la semana (spec §8) cierra la posición. Se reabre en la **primera decisión permitida** del día hábil siguiente (cierre de la primera vela de 15m, entrada en la apertura de la segunda) si ese día sigue dentro de la ventana; es una operación nueva, con su propio paso por el riesgo y su stop recalculado con el ATR de ese día.
  - Si el último día hábil del mes es el último de su semana (viernes, o jueves con el viernes feriado), la spec no deja abrir en sus últimos 30 minutos: la ventana empieza con la reapertura del día hábil siguiente.
  - Si la entrada del último día no está permitida por otro motivo (calidad de datos, vencimiento), también se pasa a la primera decisión permitida siguiente dentro de la ventana.
- Si salta el stop, no se reabre hasta el mes siguiente.
- Un evento cuya salida cae en el fuera de muestra no se opera (con las velas cortadas al 01-11-2025 no existe la vela de salida). Es el de octubre de 2025.

### 2.5 Retornos y estadística

- **Retorno mensual de un índice:** dentro de cada ventana, capital realizado al fin de cada mes calendario (UTC) sobre el del fin del mes anterior (o el inicial). El P/L de una operación cuenta en el mes en que se cierra.
- **Cartera:** promedio simple de los tres retornos mensuales del mismo mes (153 meses).
- **Criterio 1:** prueba t de Student de una muestra, unilateral (H0: media ≤ 0), sobre los 153 retornos mensuales de la cartera. Pasa si p < 0,0125 **con los dos escenarios** de financiación.
- **Criterio 2:** Sharpe deflactado de Bailey y López de Prado (2014) sobre los mismos retornos mensuales (Sharpe mensual sin anualizar, asimetría y curtosis de la muestra):
  - `SR0 = sqrt(V) × ((1 − γ) Φ⁻¹(1 − 1/N) + γ Φ⁻¹(1 − 1/(N e)))`, con γ = 0,5772 (Euler–Mascheroni).
  - **N = 350** pruebas registradas hasta ahora, redondeando hacia arriba: ~300 cortes de #461, 11 variantes del DE40 (#373), ML v1 y las tres familias de ML v2 (#430, #435), los walk-forward de la estrategia actual (DE40; US500, US100 y US30 con dos tasas; US2000 con cuatro combinaciones) y las 2 familias de esta ronda.
  - **V = 1 / (T − 1)**, la varianza del Sharpe estimado bajo la hipótesis nula, porque los Sharpe de las pruebas anteriores no están guardados en una misma unidad. T = 153.
  - `DSR = Φ((SR − SR0) × sqrt(T − 1) / sqrt(1 − γ3 × SR + (γ4 − 1) / 4 × SR²))`, con γ3 la asimetría y γ4 la curtosis (no el exceso). Pasa si DSR ≥ 0,95 con los dos escenarios.
  - Se informa también, solo como referencia, con N = 100 y N = 1000.
- **Criterio 3:** media de los retornos mensuales de la cartera > 0 en 02-2013 → 12-2019 **y** en 01-2020 → 10-2025, **y** media de los retornos mensuales > 0 en al menos 2 de los 3 índices en toda la validación. Con los dos escenarios.
- **Criterio 4:** con los **costos × 1,5** (spread, slippage, comisión y financiación neta de la base, todos juntos: `neto estresado = P/L a precio medio − 1,5 × costos`), sobre las mismas operaciones y tamaños, la media de los retornos mensuales de la cartera sigue > 0. Con los dos escenarios.
- La familia **pasa** solo si cumple los cuatro criterios. Si no, no pasa. **No concluyente** solo si la corrida no pudo hacerse (por ejemplo, faltan datos).
- Las estadísticas (prueba t, Φ, Φ⁻¹, asimetría, curtosis) se calculan con `f64`: no son dinero. Todo lo que es dinero (precios, P/L, costos, capital) usa `Decimal`.

### 2.6 Descriptivo, sin voto

- US2000 con los costos supuestos «proporcionales» de #421 (`docs/analysis/2026-11-us2000-supuestos-proporcional.toml`), validación desde 08-2019. DE40 no (sin historia).
- Por familia e índice: operaciones, rechazos del riesgo por motivo, P/L en R antes y después de costos, costo medio en R y operaciones que no se habrían abierto con 10 000 €.

### 2.7 Registro de corridas

Cada corrida que se haga (aunque falle o se repita por un error de código) se anota en el informe `docs/analysis/2026-11-ronda1-orb-cambio-de-mes.md`, con su comando. Si una corrida se repite por un error de código, se dice cuál fue el error y que la regla no cambió.
