# Ronda 3 de investigación: exposición manejada por volatilidad (V) y cascada al cruzar un número redondo (M1)

Issue #493 · 24-11-2026 · programa 2: `docs/analysis/2026-11-programa-2-propuesta.md` · pre-registro: `docs/analysis/2026-11-ronda3-preregistro.md` (commit `ec77dd7` del 24-11-2026 a las 17:15 +01:00, **antes** del código, commit `dd5a4c0` a las 17:23, y de la primera corrida, a las 17:25).

> **Es un estudio, no un resultado oficial.** El tipo de referencia de la financiación de US500, US100 y US30 sigue pendiente con IG: se usan los dos escenarios de #421, solo en memoria. **El fuera de muestra (desde el 01-11-2025) no se cargó ni se miró.** Las velas son de futuros, no del CFD. Los resultados pasados no prometen ningún resultado futuro, y nada de esto es una recomendación de operar.

## Resumen para alguien que no sabe de trading

- **V** (tener más índice cuando el mercado está calmo y menos cuando está agitado) ganó un poco más por unidad de riesgo de mercado que comprar y mantener (≈ +2 % por año de "alfa"), pero **no es una diferencia que se pueda distinguir del azar**, y su rendimiento por unidad de riesgo total (Sharpe ≈ 0,85) quedó **por debajo** del de comprar y mantener (≈ 0,90). **No pasa.**
- **M1** (seguir el precio cuando cruza un número redondo como 5000) **pierde** después de costos, y antes de costos los números redondos **no se comportan distinto** de los niveles intermedios (5025, 5075…). No hay huella de la "caza de stops" en números redondos de estos índices. **No pasa.**
- **Deriva nocturna:** existe en nuestros datos (≈ +0,8 puntos básicos por día entre las 2 y las 3 de la madrugada de Nueva York, estadísticamente claro), pero es **la cuarta parte del costo** de abrir y cerrar el CFD a esa hora. No sirve con el CFD.
- **Conclusión:** en 2013–2025, comprar y mantener los índices sigue siendo lo mejor que medimos. Ninguna de las seis familias probadas en tres rondas lo superó.

## Veredicto

| Familia | Escenario EUR (EONIA→€STR) | Escenario USD (EFFR→SOFR) | Veredicto |
|---|---|---|---|
| **V. Exposición manejada por volatilidad** | pasa el 3 y el 4; no pasa el 1, el 2 ni el 5 | igual | **NO PASA. Se cierra.** |
| **M1. Cascada al cruzar un número redondo** | no pasa ninguno (1 a 5) | igual | **NO PASA. Se cierra.** |

- **V:** alfa contra comprar y mantener de +0,19 % por mes (EUR) y +0,16 % (USD), con p = 0,17 y 0,21 (hacía falta < 0,0125). Positivo en las dos mitades y en los tres índices, y aguanta los costos × 1,5, pero el Sharpe de la cartera (0,245 mensual, ≈ 0,85 anual) es **menor** que el de comprar y mantener (0,261, ≈ 0,90).
- **M1:** −0,62 % por mes en la cartera (t = −2,37). Antes de costos, +0,041 R por operación en los números redondos y **+0,047 R en el control**: el número redondo no agrega nada.
- Según la regla de #466 §4.4 (que el programa 2 mantiene), las dos familias se **cierran**: no se prueban variantes sobre estos datos.
- No hay candidato para el fuera de muestra: queda intacto.
- Presupuesto del programa 2: la ronda 3 gastó **2,5 %**; quedan **2,5 %** de reserva para la ronda 4.

## Método (resumen; el detalle está en el pre-registro)

- **Datos:** velas de 15m de los futuros ES, NQ e YM de Databento (`--fuente futuros`), del 01-01-2011 al **01-11-2025, sin cargar nada del fuera de muestra**.
- **Walk-forward** de `config/data_split.toml`: 51 ventanas de validación de 3 meses, 02-2013 → 10-2025 (153 meses), cada una con una cuenta nueva de 100 000 € por índice. **Cartera** = promedio de los retornos mensuales de los tres índices.
- **V:** en cada revisión mensual (la de la familia A), peso = `min(2, c / RV)`, con `RV` la varianza de los retornos diarios desde la revisión anterior y `c` estimado **solo con el entrenamiento** de cada ventana (misma volatilidad que sin manejar). Rebalanceo en la apertura de la vela siguiente; se mantiene los fines de semana; spread y slippage sobre lo que cambia, financiación de IG noche por noche menos la base del futuro. Comprar y mantener = el mismo cálculo con peso 1 y sin rebalanceos (da exactamente lo mismo que el de la ronda 2: hay test, y la cartera da +1,0955 % por mes y Sharpe 0,2607, igual que en la ronda 2).
- **M1:** cruce de un múltiplo de 50 (US500), 100 (US100) o 500 (US30) al cierre de una vela de 15m, en las horas permitidas de la spec §2; entrada en la vela siguiente en la dirección del cruce, stop a 2 × ATR(14) de 15m con el módulo de riesgo sin cambios (1 %), salida a las 2 horas. Control: la misma regla con los niveles intermedios.
- **Costos** de config y estrés × 1,5, con las mismas funciones que el backtest de la estrategia en uso. Conexión a la base con `default_transaction_read_only=on`.
- **Herramienta:** ejemplo `ronda3` (módulo `cfds_backtest::research`); `config/strategy.toml`, `config/risk.toml`, el runner y el paper no cambian. Parámetros en `config/research.toml` (`[vol_managed]`, `[round_number]`, `[overnight_drift]`, `[rule_round3]`).

## Resultados

### V. Exposición manejada por volatilidad

| Índice | Escenario | Peso medio | Meses en el tope (2) | Cambio medio del peso por mes | V: retorno mensual (Sharpe) | Comprar y mantener: retorno mensual (Sharpe) | α por mes | β | t de α |
|---|---|---|---|---|---|---|---|---|---|
| US500 | EUR | 0,98 | 28 de 152 | 0,47 | +0,80 % (0,221) | +0,99 % (0,245) | +0,16 % | 0,64 | 0,79 |
| US500 | USD | | | | +0,71 % (0,196) | +0,90 % (0,220) | +0,13 % | 0,64 | 0,65 |
| US100 | EUR | 0,94 | 28 de 152 | 0,49 | +1,24 % (0,283) | +1,46 % (0,293) | +0,31 % | 0,64 | 1,25 |
| US100 | USD | | | | +1,15 % (0,263) | +1,36 % (0,273) | +0,29 % | 0,64 | 1,14 |
| US30 | EUR | 1,01 | 24 de 152 | 0,46 | +0,66 % (0,169) | +0,84 % (0,203) | +0,06 % | 0,72 | 0,28 |
| US30 | USD | | | | +0,56 % (0,145) | +0,74 % (0,178) | +0,03 % | 0,72 | 0,17 |

Ninguna ventana se quedó sin `c` (siempre hubo 12 pares o más en el entrenamiento). Son 152 pesos para 153 meses: el primer mes de la validación usa el peso de la revisión de enero de 2013, que cae en el entrenamiento.

| Criterio (cartera, 153 meses) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Alfa contra comprar y mantener (HC1, unilateral, nivel 1,25 %) | α = +0,192 % por mes (≈ +2,3 % anual), β = 0,646, e.e. 0,201 %, t = 0,95, p = 0,171 | α = +0,164 %, β = 0,645, t = 0,82, p = 0,207 | no |
| 2. Sharpe deflactado de la ventaja (N = 354) | Sharpe mensual 0,077 (≈ 0,27 anual), SR0 = 0,239, DSR = 0,025 | 0,067, DSR = 0,018 | no |
| 3. Consistencia | ventaja +0,21 % por mes en 2013–2019 y +0,17 % en 2020–2025; α > 0 en 3 de 3 | +0,20 % / +0,12 %; 3 de 3 | sí |
| 4. Costos × 1,5 | α = +0,158 % por mes | α = +0,117 % | sí |
| 5. Sharpe de V ≥ el de comprar y mantener | 0,245 frente a 0,261 | 0,220 frente a 0,237 | no |

Lectura:
- **El signo es el que dice la literatura** (alfa positivo, en las dos mitades y en los tres índices), pero el tamaño es chico frente al ruido: con 153 meses, un alfa de 2 % anual tiene un error estándar casi igual.
- **El alfa positivo no alcanza para un Sharpe mejor.** V tiene β ≈ 0,65 contra comprar y mantener y una parte de su variación no viene del mercado (el ruido del peso), así que su Sharpe total queda por debajo aunque el alfa sea positivo. Una explicación posible, **no verificada**, es que en 2013–2025 varios meses agitados fueron seguidos de rebotes fuertes (por ejemplo 2020), que V tomó con poco peso.
- Coincide con la advertencia de Cederburg et al. (2020): con `c` estimado en tiempo real, la mejora sobre el mercado no es robusta.
- **Los costos no son el problema** (con × 1,5 el alfa sigue positivo): el rebalanceo mensual es barato. El problema es que no hay ventaja que se distinga del azar.

### M1. Cascada al cruzar un número redondo

Los dos escenarios dan casi lo mismo (las operaciones duran 2 horas y casi nunca cruzan el corte de la financiación).

| Índice | Grupo | Operaciones | Salen en el stop | Rechazadas por el riesgo | R medio antes de costos | Costo medio | R neto medio | Aciertos | Retorno mensual medio (EUR) | Con costos × 1,5 |
|---|---|---|---|---|---|---|---|---|---|---|
| US500 | redondos (50) | 2494 | 847 | 31 pérdida diaria, 5 drawdown | +0,020 R | 0,105 R | −0,085 R | 42,4 % | −1,39 % | −2,23 % |
| US500 | control (25, 75) | 2590 | 831 | 36, 2 | +0,046 R | 0,109 R | −0,062 R | 43,3 % | −1,07 % | −1,97 % |
| US100 | redondos (100) | 4416 | 1495 | 123, 9 | +0,043 R | 0,059 R | −0,017 R | 44,8 % | −0,50 % | −1,35 % |
| US100 | control (50) | 4516 | 1502 | 129, 5 | +0,056 R | 0,058 R | −0,002 R | 44,5 % | −0,10 % | −0,96 % |
| US30 | redondos (500) | 2107 | 688 | 29, 2 | +0,062 R | 0,061 R | +0,001 R | 44,3 % | +0,02 % | −0,40 % |
| US30 | control (250) | 2096 | 654 | 15, 0 | +0,026 R | 0,056 R | −0,030 R | 46,2 % | −0,42 % | −0,80 % |

R mediano en puntos: 13,7 (US500), 58,4 (US100) y 100,7 (US30). Con 10 000 € no se habrían abierto menos del 1 % de las operaciones.

| Criterio (cartera, 153 meses) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Prueba t unilateral, nivel 1,25 % | media −0,623 % por mes (desvío 3,25 %), t = −2,37, p = 0,991 | −0,624 %, t = −2,38 | no |
| 2. Sharpe deflactado (N = 354) | Sharpe mensual −0,19, DSR = 0,000 | igual | no |
| 3. Consistencia | 2013–2019: −1,04 % por mes; 2020–2025: −0,12 %; índices > 0: 1 de 3 | igual | no |
| 4. Costos × 1,5 | −1,32 % por mes | −1,33 % | no |
| 5. R antes de costos de los redondos > el del control | +0,041 (9017 op.) frente a +0,047 (9202 op.) | +0,041 frente a +0,047 | no |

Lectura:
- **Hay una continuación chica después de cualquier cruce** (+0,02 a +0,06 R antes de costos), pero el número redondo **no** la hace mayor: el control da lo mismo o más. Es lo que se esperaba si el efecto de Osler (en divisas, con datos de hace más de 20 años) no existe en estos futuros, o si lo que se mide es solo el impulso de corto plazo de cualquier ruptura.
- Los costos (0,06–0,11 R) se llevan esa continuación. Pasa lo mismo que con C, B y la estrategia de 15m: en un horizonte de horas, el costo del CFD es del tamaño de lo que se puede capturar.
- **No reabre la puerta a la "caza de stops"**: lo que el informe de #492 proponía como la versión mejor fundada no muestra la huella del número redondo.

### Deriva nocturna (descriptivo sin voto)

Retorno medio de 02:00 a 03:00 ET, días hábiles de la validación (02-2013 → 10-2025):

| Índice | Días | Media (puntos básicos) | t | Anualizado (× 252) | Costo de ida y vuelta del CFD a esa hora* |
|---|---|---|---|---|---|
| US500 | 3276 | +0,83 | 3,15 | +2,1 % | ≈ 1,9 puntos ≈ 3–4 pb |
| US100 | 3270 | +1,03 | 3,42 | +2,6 % | ≈ 3,2 puntos ≈ 1,5–2,2 pb |
| US30 | 3276 | +0,63 | 2,49 | +1,6 % | ≈ 6,2 puntos ≈ 1,5–1,8 pb |
| US2000 (desde 2019-08) | 1594 | +0,38 | 0,69 | +1,0 % | sin dato de IG |

\* Spread de IG de 07–08 UTC (`docs/research/ig-spreads-2026-11.md`: 1,6 / 2,5 / 5,0 puntos) más el slippage de la spec (0,1 × spread + 1 tick por ejecución), sobre precios del índice de 2020–2025. Es una cuenta aproximada, no un backtest.

- **El efecto existe en nuestros datos** (t de 2,5 a 3,4), con la mitad o dos tercios del tamaño publicado (1,48 pb por día en el artículo, con otra muestra).
- **Con el CFD no se puede:** el costo es entre 2 y 4 veces el efecto en todos los índices. Solo volvería a ser candidato con un vehículo mucho más barato (propuesta §1.4), y aun así el margen sería chico.

### Descriptivo, sin voto: US2000 (solo V, costos supuestos)

Validación 08-2019 → 10-2025 (75 meses, la "primera mitad" son 5 meses). V pierde frente a comprar y mantener: α = −0,47 % por mes (EUR) y −0,50 % (USD), t ≈ −1,1 a −1,2; Sharpe de V −0,001 / −0,020 frente a 0,097 / 0,079 de comprar y mantener. M1 no se corrió en US2000 (no tiene paso en el informe de #492).

## Registro de corridas (pre-registro §7)

Todas con el binario compilado del commit `dd5a4c0`, contra la base en solo lectura, sin cargar velas desde el 01-11-2025:

| # | Hora (24-11-2026, Madrid) | Comando (`target/release/examples/ronda3 …`, desde `engine/`) | Nota |
|---|---|---|---|
| 1 | 17:25 | `US500 US100 US30 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida … --operaciones …` | primera corrida |
| 2 | 17:25 | `US500 US100 US30 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida … --operaciones …` | |
| 3 | 17:26 | `US2000 --descriptivo --supuestos ../docs/analysis/2026-11-us2000-supuestos-proporcional.toml --escenario-financiacion EUR=…` | descriptivo |
| 4 | 17:26 | lo mismo con `USD=…` | descriptivo |

Ninguna corrida se repitió. Esta ronda suma **2 pruebas** (V y M1) al registro para el Sharpe deflactado: el total queda en **354**. El control de M1 y la deriva nocturna son parte de la misma corrida y no suman pruebas.

Los retornos mensuales (V, comprar y mantener, M1 y su control, con y sin estrés) quedan en `docs/analysis/2026-11-ronda3-retornos-eur.csv` y `docs/analysis/2026-11-ronda3-retornos-usd.csv`.

## Verificación

- Revisión a mano de operaciones de M1 en el registro exportado:
  - US500, 04-02-2013: la vela que cierra a las 10:00 ET cierra en 1501 (cruzó 1500 hacia arriba): largo en la apertura de la siguiente, stop 1501 − 2 × ATR (1,633) = 1497,73, salió en el stop. Tamaño 306,1 = 1000 € de riesgo / 3,27 puntos.
  - US500, 05-02-2013: cruce de 1500 a las 09:45 ET, entrada en la vela siguiente, salida exactamente 2 horas después del cierre de la vela del cruce; costo 1,1 puntos × 380,88 = 418,97.
  - Control, 19-02-2013: cruce de 1525 (nivel intermedio de 50).
- Comprar y mantener calculado con `managed_returns` (peso 1) da la misma cartera que la ronda 2 (+1,0955 % por mes, Sharpe 0,2607 en EUR).
- 11 tests nuevos (`engine/crates/backtest/tests/research_round3.rs`), y los de las rondas 1 y 2 sin cambios:
  - configuración de la ronda 3 y rechazo de valores malos;
  - alfa HC1 contra un cálculo hecho aparte (coeficientes, error estándar y t);
  - V: la varianza realizada suma los retornos diarios desde la revisión anterior (con el cambio de revisión al lunes cuando el mes termina en viernes); con volatilidad constante el peso es 1 y el tope se aplica; con peso 1 sin rebalanceos es **exactamente** comprar y mantener (con y sin vencimiento); el peso escala la exposición y el rebalanceo se ejecuta en la apertura siguiente (no gana un salto decidido en su propia vela); el estrés baja el retorno;
  - M1: cruce alcista y bajista, tocar sin cruzar, control sin cruce, hueco, vencimiento, decisión bloqueada, horizonte corto, sin ATR, y **no repintado**;
  - deriva nocturna: 02:00–03:00 ET con el cambio de horario, sin sábados, vela faltante y vencimiento.

## Desvíos y límites

1. **Ningún desvío del pre-registro.** No hubo que precisar nada después del commit del pre-registro.
2. **V sin stop y con tamaño por exposición.** Es una prueba de exposición como comprar y mantener: no pasa por el módulo de riesgo (pre-registro §3.1). Como no pasó, no hace falta discutir el cambio de spec.
3. **Futuros, no el CFD.** Los números redondos de M1 son los del **futuro**, que difieren de los del contado y del CFD por la base (decenas de puntos en ES). Si las órdenes de los clientes de IG se juntan en los números redondos del CFD, M1 no las miró. Con los datos que tenemos no se puede (no hay velas del CFD de 15 años).
4. El spread es el típico de config, no uno medido en la historia (#267); el tipo de referencia de la financiación sigue pendiente y los dos escenarios dan el mismo veredicto.
5. **Sin calendario de noticias** para el período, igual que en las rondas anteriores.

## Para el dueño

- V y M1 **no pasan** y quedan cerradas para estos datos. No se activa nada, no se toca la estrategia en uso y el fuera de muestra sigue intacto.
- **Seis familias en tres rondas** (C, D1, A, B, V, M1) y la estrategia de 15m: ninguna supera los costos o a comprar y mantener. Las tres con el signo esperado (D1, A, V) tienen efectos del orden de 0,03–0,05 R por operación o ≈ 2 % anual, chicos frente al ruido de 13 años.
- **Lo que queda del programa 2 (2,5 % de reserva)** conviene gastarlo en un **tipo de dato distinto**, no en otra regla de precios sobre las mismas velas. En orden (propuesta §4):
  1. **Familia E (eventos programados: Fed, CPI, empleo):** necesita cargar el calendario histórico oficial 2011–2025 (Issue de datos, público y chico).
  2. **M2 (barrida frente a ruptura del día anterior):** primero el descriptivo que pide #492, sin gastar presupuesto.
  3. **Acciones sueltas (tipo NVDA):** solo si decidís cambiar el universo y pagar datos (precios con empresas que salieron del índice, fechas de resultados, noticias con hora).
- **La palanca más grande que vimos no es una señal sino el costo**: para algo que se mantiene, la tasa administrativa del CFD (3 % anual) pesa del orden de 0,25 % por mes frente a un retorno de ≈ 1 % por mes de comprar y mantener. Verificar las tarifas oficiales de un broker de futuros (propuesta §1.4) no gasta presupuesto de error. Cualquier cambio de producto o de broker es un ADR, y lo de dinero real sigue bloqueado (`CLAUDE.md` §5).
