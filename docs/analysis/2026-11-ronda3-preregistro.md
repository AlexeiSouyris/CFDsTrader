# Pre-registro de la ronda 3: exposición manejada por volatilidad (V) y cascada al cruzar un número redondo (M1)

Issue #493 · 24-11-2026 · base: `docs/analysis/2026-11-programa-2-propuesta.md` (programa 2, §3 y §4) · método: el de las rondas 1 y 2 (`2026-11-ronda1-preregistro.md`, `2026-11-ronda2-preregistro.md`).

Este archivo se commitea **antes de correr nada contra la base**. La hora del commit que lo agrega es la del pre-registro. Cambiar algo de acá después de correr invalida la familia y la cuenta como una prueba nueva.

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro.

## 1. Lo que viene de las rondas anteriores sin cambios

- Walk-forward de `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3). Resultado = **concatenación de las ventanas de validación** (esperado: 02-2013 → 10-2025, 51 ventanas, 153 meses).
- **El fuera de muestra (desde el 01-11-2025) no se carga, no se mira y no se usa.** Las velas se piden solo hasta su inicio; para planificar la separación se lee solo la hora de la última vela guardada (función `load` de las rondas 1 y 2).
- Velas de 15m de los futuros de Databento (ES, NQ, YM), `--fuente futuros`, con la preparación común (`examples/common/prepare.rs`: validación, horario de la spec §2 y §7.1, vencimientos con cierre forzado, base del futuro de #470). Conexión con `default_transaction_read_only=on`.
- Costos: spread típico y slippage de config (`fill_price`), comisión, financiación de IG **noche por noche** con los dos escenarios empalmados de #421 (EUR: `2026-11-eur-eonia-estr.csv`; USD: `2026-11-usd-effr-sofr.csv`), **menos la base del futuro** (#470), y el estrés × 1,5. Las mismas funciones de costos del backtest de la estrategia en uso.
- Una cuenta de **100 000 €** por índice, nueva en cada ventana (mismo motivo que la ronda 2 §1: que el tamaño mínimo no decida la prueba). **Cartera** = promedio simple de los retornos mensuales de US500, US100 y US30. Retornos mensuales en UTC. Estadística en `f64`, dinero en `Decimal`.
- US2000 se informa aparte, **descriptivo y sin voto**, con los costos supuestos «proporcionales» de #421, solo para V.

## 2. Presupuesto y unidad de prueba

- Programa 2: 5 %. Ronda 3: 2,5 % entre **dos** familias (V y M1): **1,25 % unilateral por familia**. Quedan 2,5 % de reserva (propuesta §3.1).
- Sharpe deflactado con **N = 354** (las 352 registradas hasta la ronda 2, más V y M1). Mínimo 0,95. Referencia también con N = 100 y N = 1000.
- Segunda mitad de la validación desde 2020-01-01.

## 3. Familia V: exposición manejada por volatilidad (Moreira y Muir, 2017)

### 3.1 Definición

- **Revisión mensual:** la misma de la familia A (`monthly_reviews`: primera decisión permitida desde el cierre de la anteúltima vela de 15m del último día hábil del mes). Sea `r_k` el índice de la vela de la revisión `k`.
- **Varianza realizada de la revisión `k`:** `RV_k = Σ g_j²`, donde `g_j` son los retornos **encadenados en los vencimientos** (`chained_return`) entre puntos consecutivos de la lista: `r_{k−1}`, las velas de cierre de la sesión de contado de cada día con sesión estrictamente entre `r_{k−1}` y `r_k`, y `r_k`. Es la varianza de los retornos diarios desde la revisión anterior (≈ el mes anterior), conocida al cierre de `r_k`. Sin revisión anterior o con `RV_k = 0`: sin peso.
- **Constante `c`** (una por ventana de validación e índice, **solo con el entrenamiento**): con las revisiones `k` cuyo cierre cae en el entrenamiento y que tienen `RV_{k−1}`, sea `R_k` = retorno encadenado de `r_{k−1}` a `r_k`. Entonces `c = desvío(R_k) / desvío(R_k / RV_{k−1})`: la misma volatilidad que sin manejar, como en el artículo, pero estimada sin mirar la validación. Si hay menos de 12 pares, esa ventana no se corre (se informa).
- **Peso:** `w_k = min(tope, c / RV_k)`, con **tope = 2** (fijado por nosotros, no viene del artículo: limita el apalancamiento a 2 veces el capital; está dentro del máximo 10:1 de `config/risk.toml`). Solo largos.
- **Ejecución en cada ventana de validación** (igual que comprar y mantener de la ronda 2 §2.2, pero con peso variable):
  - entra en la apertura de la primera vela de la sesión de contado de la ventana con el peso `w_j` de la última revisión `j` cerrada **antes** de esa apertura (puede ser del entrenamiento: se conocía);
  - en cada revisión `k` cuyo cierre cae dentro de la ventana, antes de la última vela, la posición pasa a `w_k` en la **apertura de la vela siguiente** (`t + 1`);
  - sale al cierre de la última vela de la sesión de contado de la ventana;
  - unidades: al entrar y en cada rebalanceo, `unidades = w × valor de la cuenta / precio de ejecución (medio)`, con el valor de la cuenta en la moneda del instrumento (el capital inicial = el precio de entrada, como en comprar y mantener);
  - **se mantiene los fines de semana** y en los vencimientos se encadena sin costo (el CFD no rola), igual que comprar y mantener.
- **Costos:** en la entrada, en cada rebalanceo (sobre las unidades que cambian) y en la salida: spread + slippage (`fill_price`) y comisión; financiación de cada corte sobre `unidades × cierre de la última vela anterior`, menos la base del futuro. Estrés: todos los costos × 1,5.
- **Comprar y mantener** para los criterios: la misma función con peso 1 todo el tiempo (debe dar lo mismo que `buy_and_hold_returns` de la ronda 2; hay test).
- **Sin stop.** Como comprar y mantener, V es una prueba de **exposición**: no pasa por `cfds_risk::check` (que dimensiona por 1 % de riesgo con stop). Si V pasara, operarla pide un cambio de spec aprobado aparte (tamaño por exposición, fin de semana, stop de catástrofe); la prueba no lo habilita.

### 3.2 Criterios (todos, con los dos escenarios de financiación)

Sea `y` la serie mensual de la cartera de V y `x` la de comprar y mantener (153 meses).

1. **Alfa:** regresión por mínimos cuadrados `y = α + β x + e`, error estándar de `α` robusto a heterocedasticidad (White, HC1: factor `n / (n − 2)`), prueba t unilateral (H0: α ≤ 0) con `n − 2` grados de libertad. Pasa si p < **0,0125**.
2. **Sharpe deflactado de la ventaja:** la serie `α + e_t = y_t − β x_t`, con la fórmula de la ronda 1 §2.5 y N = 354. Pasa si DSR ≥ 0,95.
3. **Consistencia:** media de `y_t − β x_t` > 0 en 02-2013 → 12-2019 **y** en 01-2020 → 10-2025 (con el `β` de toda la muestra), **y** alfa > 0 en al menos 2 de los 3 índices (cada índice con su propia regresión).
4. **Costos × 1,5:** el alfa de la misma regresión con `y` y `x` estresados (los dos) sigue > 0.
5. **Sharpe mensual de la cartera de V ≥ el de comprar y mantener.**

V **pasa** solo si cumple los 5 con los dos escenarios.

### 3.3 Descriptivo

Por índice: peso medio, meses en el tope, cambio medio del peso por mes, retorno mensual medio y Sharpe de V y de comprar y mantener, alfa y β por índice.

## 4. Familia M1: cascada al cruzar un número redondo (Osler, 2003 y 2005; informe de #492)

### 4.1 Definición

- **Paso `S`** (del informe de #492 §4.5, fijado antes de mirar): **US500 50, US100 100, US30 500** puntos del futuro. Nivel redondo `R = k × S`; nivel de control `R' = k × S + S/2`.
- **Cruce** en la vela `t` (cerrada): la vela `t − 1` termina justo donde empieza `t` (sin hueco), no hay un vencimiento entre las dos, y
  - alcista: `C[t−1] < R ≤ C[t]` para algún `R`;
  - bajista: `C[t−1] > R ≥ C[t]` para algún `R`.
  - Umbral `e = 0` (el informe deja `e` en config sin valor; cero evita un parámetro libre). Si una vela cruza varios niveles, es un solo cruce.
- **Decisión** al cierre de `t`, solo si es una decisión permitida de la preparación (horario de la spec §2: sesión de contado de XNYS desde los 15 minutos después de la apertura; sin abrir en los últimos 30 minutos del último día de la semana; vencimientos). **Entrada** en la apertura de `t + 1`, en la dirección del cruce.
- **Stop:** 2 × ATR(14) de 15m en la vela `t` (Wilder, como la familia B); el riesgo mide con ese mismo ATR. Pasa por `cfds_risk::check` con `config/risk.toml` sin cambios (1 % por operación, pérdida diaria 3 %, kill switch).
- **Salida:** al cierre de la última vela que termina **2 horas** (`h = 2 h`, el ejemplo del informe) o menos después del cierre de `t`. Antes, si salta el stop o hay un cierre forzado (fin de semana, vencimiento). Si no hay ninguna vela después de `t + 1` en ese plazo, no se opera.
- Una posición a la vez por índice (`run_plans`): un cruce mientras hay una posición abierta no se opera.
- **Control:** la misma regla, idéntica, con los niveles `R'` en vez de `R`, ejecutada aparte (otra cuenta). Solo para el criterio 5.

### 4.2 Criterios (cartera, con los dos escenarios)

1. Prueba t unilateral de los retornos mensuales netos de la cartera de M1 (niveles redondos), p < **0,0125**.
2. Sharpe deflactado ≥ 0,95 con N = 354.
3. Consistencia: media > 0 en las dos mitades y en al menos 2 de 3 índices.
4. Costos × 1,5: media mensual > 0.
5. **Control de #492:** el R medio **antes de costos** de todas las operaciones de M1 (los tres índices juntos) es **mayor** que el de las operaciones del control.

M1 **pasa** solo si cumple los 5 con los dos escenarios.

## 5. Descriptivo sin voto: deriva nocturna (Boyarchenko, Larsen y Whelan, 2023)

Por índice, en los días de la validación (02-2013 → 10-2025): retorno encadenado desde la apertura de la vela de 15m que empieza a las **02:00 ET** hasta el cierre de la que termina a las **03:00 ET** (America/New_York), solo días hábiles con las cuatro velas. Se informa la media en puntos básicos, su t y la cantidad de días, junto al costo de ida y vuelta del CFD a esa hora (spread de IG de 07–08 UTC, `ig-spreads-2026-11.md`). **No tiene regla ni voto y no gasta presupuesto** (la propuesta §1.5 ya muestra que con el CFD no puede pasar).

## 6. Parámetros en config

Los valores de esta ronda quedan en `config/research.toml`, en secciones nuevas (`[vol_managed]`, `[round_number]`, `[overnight_drift]`, `[rule_round3]`), validado al cargar. Las secciones de las rondas 1 y 2 no cambian. Solo los lee el ejemplo nuevo `ronda3`. `config/strategy.toml`, `config/risk.toml`, el runner y el paper no cambian.

## 7. Registro de corridas

Cada corrida (aunque falle o se repita por un error de código) se anota en el informe de la ronda 3 con su hora y su comando. Si una corrida se repite por un error de código, se dice cuál fue el error y que la regla no cambió.

## 8. Aprobación

El pedido del dueño en #493 abre el programa. Si el dueño no aprueba el presupuesto o las familias de este pre-registro, los resultados de la ronda 3 se toman como **no válidos** y se vuelve a pre-registrar (propuesta §5).
