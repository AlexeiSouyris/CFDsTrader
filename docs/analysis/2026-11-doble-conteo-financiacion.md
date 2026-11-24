# Estudio: ¿el backtest con futuros cuenta dos veces la financiación?

Issue #467 · 24-11-2026 · **solo informe, en lectura**: no cambia `config/`, ni código, ni usa el fuera de muestra. Sigue al hallazgo de la propuesta de investigación (#462, PR #466, §2.3). Si el cálculo hay que cambiarlo, el cambio se propone acá (§6) y va en un PR aparte, con tests.

## Conclusión

1. **Sí, hay doble conteo, y es la tasa del futuro (no los dividendos).** El precio del futuro ya descuenta la tasa de interés menos los dividendos (la base). El backtest con `--fuente futuros` le suma además la financiación completa del CFD (referencia ± 3 %). Para un CFD de índice al contado, IG **abona los dividendos a los largos y se los cobra a los cortos**. Por eso los dividendos quedan bien contados (el futuro los trae en la base y el CFD los recibe como ajuste). Lo que sobra es **una tasa de interés por noche**:
   - a los **largos** se les cobra de más ≈ tasa del futuro × nocional / 360 por noche;
   - a los **cortos** se les cobra de menos (reciben de más) por la misma cantidad.
2. **En la estrategia actual pesa muy poco.** El 30–31 % de las operaciones pasa una noche (casi siempre una sola; el máximo es 3). Con 15 años de futuros (los mismos bloques de #461):
   - el sesgo neto es de **+0,0013 a +0,0018 R por operación** (+406 a +551 € sobre 2988–3177 operaciones), frente a un neto de −0,13 a −0,20 R;
   - corregido, el R medio neto pasa de −0,200 a −0,198 (US500), de −0,128 a −0,127 (US100) y de −0,157 a −0,156 (US30). **No cambia ninguna conclusión** de #405, #421, #456 ni #461.
3. **En la familia A (tendencia de baja frecuencia) pesa mucho.** Un largo que se mantiene meses paga de más la tasa del futuro sobre todo el nocional: en promedio **1,7 % anual** del nocional en 2013–2025 y **4,0 %** en 2022–2025 (EFFR→SOFR). Con el stop de ~3 ATR diario de la propuesta, eso es **≈ 0,4 R por año de largo en promedio y ≈ 1,0 R por año en 2022–2025** (tenencia continua). Con la corrección, la financiación de un largo queda en `ref − r + 3 %` del nocional por año: si IG usa la tasa USD como referencia, **≈ 3 %, solo la tasa administrativa**, en lugar de los 5–7 % que suponía #466. Lo mismo vale, en menor medida, para D1 (cambio de mes) y E (reversión de varios días).
4. **Comprar y mantener también está sesgado.** Es un largo de 3 meses: su retorno en los informes de walk-forward queda subestimado en ≈ tasa × nocional × 0,25. Con el tamaño mínimo (1 contrato en US500, ~4400 € de nocional sobre 10 000 €), son ≈ 0,2 puntos de retorno por ventana en promedio y ≈ 0,4 en 2022–2025. Es chico, pero va siempre en contra de comprar y mantener.
5. **Propuesta para el PR de código (§6):** con velas de futuros, restar en cada noche la tasa del futuro (EFFR→SOFR para los futuros de CME, €STR para el FDAX) con el signo del lado. Con velas de IG no se toca nada.

## 1. El mecanismo

Notación por año: `S` contado, `F` futuro, `r` tasa de la base del futuro (USD para ES, NQ, YM y RTY; EUR para FDAX), `q` dividendos, `ref` tipo de referencia de IG, `a` = 3 % de tasa administrativa.

| | Largo, por año de tenencia (en fracción del nocional) |
|---|---|
| Futuro (dentro de un contrato) | variación del contado + `q` − `r` (el futuro converge al contado: la base, `r − q`, se va consumiendo) |
| CFD de IG al contado | variación del contado + `q` (ajuste por dividendo) − (`ref` + `a`) |
| Backtest hoy (precio del futuro + financiación del CFD) | variación del contado + `q` − `r` − (`ref` + `a`) |
| **Backtest − CFD** | **−`r`** (simétrico para el corto: **+`r`**) |

- La "variación del contado" incluye la caída del índice en cada fecha ex-dividendo. El CFD la compensa con el ajuste de IG; el futuro no cae ese día porque el dividendo ya estaba en la base. Por eso los dividendos **no** son la parte doble.
- Si IG **no** ajustara dividendos, el sesgo sería `−(r − q)` en lugar de `−r`. La fuente de abajo dice que sí los ajusta.
- **DE40:** el DAX es un índice de rendimiento (reinvierte los dividendos en el propio índice), así que ahí `q` no entra en el precio y la base del FDAX es esencialmente `r` en EUR. El sesgo también es `−r`.
- Intradía no hay sesgo: sin noche, no hay financiación y la base casi no se mueve en unas horas.
- El backtest ya cierra antes de cada vencimiento (#267), así que el salto del rollover no entra en el P/L. El sesgo está en la **deriva** dentro de cada contrato.

## 2. Qué hace IG con los dividendos (fuentes oficiales)

- **IG España, "Nuestros costes"** (`https://www.ig.com/es/nuestros-costes`, leída el 24-11-2026): da la fórmula de financiación de los índices al contado, "(tipo de interés de referencia correspondiente +/- tasa administrativa)) / 360", con un 3 % de tasa. Los largos pagan y los cortos reciben. **No describe el ajuste por dividendos.** Para los CFD de índices con vencimiento (basados en futuros) dice que no hay financiación nocturna: "el coste de financiación se configura a través del spread".
- **IG, ayuda internacional** (`https://www.ig.com/en/help-and-support/cfds/market-details/how-do-dividend-adjustments-affect-my-cfd-position`, leída el 24-11-2026): en los CFD de índices al contado, cuando un dividendo baja el índice, IG compensa la cuenta para que no haya un efecto material: **abona a los largos y carga a los cortos** los puntos que el dividendo le quita al índice, por el tamaño. El ejemplo de cortos es con "Wall Street Cash" (el US30). Además, IG publica una previsión de esos ajustes para los índices principales.
- **Pendiente:** la página de IG España no lo dice explícitamente. Conviene confirmarlo con IG España (o verlo en el histórico de movimientos de la cuenta demo, sin gastar cupo de precios) antes del PR de código. Si IG España no ajustara dividendos, la corrección cambia de `r` a `r − q` (§6).

## 3. La base existe y tiene el tamaño esperado

Para confirmar que el futuro trae la tasa, se midió el salto en cada cambio de contrato del continuo (`futures_bars_1m`, último 1m del contrato viejo → primer 1m del nuevo, anualizado ×4) antes del fuera de muestra (2011-01 → 2025-10). Es ruidoso, porque el cambio cae en un fin de semana (57 horas sin barras) y el mercado se mueve, así que se muestra la mediana por período:

| Futuro | 2011–2021: mediana | 2022–2025: mediana | Cambios |
|---|---|---|---|
| ES | −2,30 % | +3,53 % | 44 / 15 |
| NQ | −1,54 % | +4,56 % | 44 / 15 |
| YM | −2,72 % | +2,96 % | 44 / 15 |
| RTY | +0,36 % | +1,61 % | 18 / 15 |
| **Tasa USD media** (EFFR→SOFR, `2026-11-usd-effr-sofr.csv`) | **0,58 %** | **4,02 %** | |

Con tasas casi en cero (2011–2021), el contrato nuevo cotiza **por debajo** del viejo (la base es negativa: dividendos > tasa). Con tasas del 4–5 % (2022–2025), cotiza **por encima**. El cambio entre períodos (+4 a +6 puntos) es del orden del cambio de la tasa (+3,4). Es lo que predice `r − q`. No es una estimación precisa de la base (para eso harían falta dos contratos a la vez, ver §6), pero confirma el mecanismo.

## 4. Cuánto pesa en la estrategia actual

**Método.** Se corrió el diagnóstico de #461 (`diagnostico --fuente futuros --tramos`, escenario EONIA→€STR, bloques trimestrales antes del fuera de muestra, conexión de solo lectura) con un volcado temporal de las operaciones a un CSV local. El volcado no se commitea; en el PR de código se agregaría como opción del ejemplo. La financiación total coincide con la de #461 (2961, 2414 y 3467 €). Para cada salida parcial, el sesgo es la suma, en cada corte de las 23:00 de Madrid que cruzó, de `fracción × tamaño × precio de entrada × r / 100 / 360`: es la misma fórmula que el motor (`funding_over`), con `r` = EFFR→SOFR, vigente desde 2 días antes del corte, y signo + en largos y − en cortos.

| Índice | Operaciones | Pasan la noche | Noches (1 / 2 / 3) | Financiación cobrada | Sesgo en largos | Sesgo en cortos | **Sesgo neto** | Por operación | R medio neto hoy → corregido |
|---|---|---|---|---|---|---|---|---|---|
| US500 | 2988 | 920 (30,8 %) | 891 / 26 / 3 | 2962 € | +994 € | −443 € | **+551 €** | +0,0018 R | −0,200 → −0,198 |
| US100 | 3080 | 923 (30,0 %) | 888 / 32 / 3 | 2414 € | +759 € | −334 € | **+425 €** | +0,0013 R | −0,128 → −0,127 |
| US30 | 3177 | 991 (31,2 %) | 957 / 31 / 3 | 3467 € | +1021 € | −615 € | **+406 €** | +0,0013 R | −0,157 → −0,156 |

- "Sesgo" = lo que el backtest cobra de más (+) o de menos (−) frente al CFD real. Corregir es **sumarlo** al neto.
- En las operaciones que pasan la noche, el sesgo es ≈ 0,01–0,017 R en un largo y ≈ −0,01 a −0,015 R en un corto. Como hay más largos que cortos que pasan la noche (≈ 60/40), queda un neto chico en contra.
- **Representa un 13–23 % de la financiación cobrada, pero la financiación entera es el 5–10 % de los costos** (#461). El sesgo es ≈ 1 % del resultado neto. No cambia ningún veredicto.
- R medio = pérdida si toca el stop, ≈ 100 € en los tres. "R medio neto" acá es neto total / operaciones / R medio, por eso US500 da −0,200 y no el −0,19 de #461, que promedia el R de cada operación.

## 5. Cuánto pesa en las familias de varios días (#466)

El sesgo por año de tenencia es `r` sobre el nocional. En R, se multiplica por nocional / R = precio / distancia del stop.

| Familia (#466) | Tenencia | Stop supuesto (US500) | Nocional / R | Sesgo por noche | Sesgo por año de largo: `r` medio 2013–2025 (1,69 %) | con `r` 2022–2025 (4,02 %) |
|---|---|---|---|---|---|---|
| A. Tendencia | meses | 3 × ATR diario ≈ 170 puntos | ≈ 26 | 0,0012 R | ≈ 0,44 R (≈ 0,25 R cerrando los viernes) | ≈ 1,0 R (≈ 0,6 R cerrando los viernes) |
| D1. Cambio de mes | ~4 noches | 2 × ATR diario ≈ 115 | ≈ 38 | 0,0018 R | ≈ 0,007 R por operación | ≈ 0,017 R por operación |
| E. Reversión | 2–5 noches | 2 × ATR diario ≈ 115 | ≈ 38 | 0,0018 R | ≈ 0,004–0,009 R por operación | ≈ 0,008–0,021 R por operación |
| Actual (15m) | 0–1 noche | ~10 puntos | ≈ 250–360 | 0,012–0,017 R | — | — |

(Precio mediano 2020–2025 ≈ 4373. Sesgo por noche calculado con `r` = 1,69 %; con 4,02 % es ≈ 2,4 veces más.)

- **En A es decisivo.** En #466 el costo de un largo se estimó en ≈ 4,2 % del nocional por año: ≈ 2,9 % de financiación (208 noches a 5 %) más ≈ 1,3 % de spread por las reaperturas de los lunes. Con la corrección, la financiación baja en `r` × 208 / 360: ≈ 1,0 punto porcentual con `r` medio y ≈ 2,3 con `r` de 2022–2025. Si la referencia de IG resulta ser la tasa USD, queda solo la tasa administrativa (≈ 1,7 % por año con el cierre de los viernes). Sin la corrección, el backtest le cobraría a los largos de A entre 1,5 y 4 veces lo real y les regalaría la misma tasa a los cortos. El resultado de A con el cálculo de hoy **no serviría**.
- En D1 y E el sesgo por operación es chico (≈ 0,005–0,02 R), pero va siempre en el mismo sentido (solo largos) y es del orden del efecto que se busca (≈ 0,03 R de costo total en #466). También hay que corregirlo antes de correrlas.
- **Comprar y mantener** de los walk-forward (#405, #421) es un largo de 3 meses: subestimado ≈ `r` × 0,25 × nocional. Con 1 contrato de US500 (~4400 € de nocional sobre 10 000 €), son ≈ 0,19 puntos de retorno por ventana con `r` de 1,69 % y ≈ 0,44 con 4,02 %. No cambia las conclusiones (la estrategia queda muy por debajo), pero el "supera a comprar y mantener" está levemente inflado a favor de la estrategia.

## 6. Propuesta de cambio del cálculo (para un PR aparte, con tests)

**Recomendada: restar la tasa de la base noche por noche, solo con velas de futuros.**

- En `funding_over` (o en una función nueva al lado), cuando la fuente es `futuros`: para cada corte, en lugar de `±(ref) + a` usar `±(ref − r_base) + a`, con `r_base` la tasa vigente en ese corte (misma regla de publicación rezagada, sin look-ahead):
  - futuros de CME (ES, NQ, YM, RTY): serie USD (EFFR hasta 2018-03 y SOFR desde 2018-04), ya bajada en `docs/analysis/2026-11-usd-effr-sofr.csv` (#421); habría que cargarla en `reference_rates` como serie oficial, con su fuente;
  - FDAX: €STR (y EONIA antes de 2019-10), que ya está en `reference_rates`;
  - qué serie corresponde a cada futuro va en config (por ejemplo en `config/candle_sources.toml` o `config/databento.toml`), con su fuente, nada hardcodeado.
- Con velas de IG (el CFD de verdad) no cambia nada: el precio del CFD sigue al contado.
- Si IG usa como referencia de los CFD de EE. UU. la misma tasa USD, la financiación neta queda en ≈ ±3 % (solo la tasa administrativa). Si usa €STR, queda `€STR − r_USD ± 3 %`. Esto depende del tipo de referencia que **todavía está pendiente** en `config/instruments.toml`.
- **Dividendos:** no se tocan, si IG España confirma que ajusta dividendos como dice su ayuda internacional. Si no los ajusta, se resta `r − q` en lugar de `r`, y hace falta una serie de dividendos con fuente.
- **Comprar y mantener** tiene que usar la misma corrección.
- **Tests:** un largo y un corto sintéticos de N noches con tasa constante (la corrección es exactamente `N × r × nocional / 360` con el signo del lado); una noche sin serie publicada frena con error, igual que hoy; con velas de IG el resultado es idéntico al actual.

**Alternativas descartadas por ahora:**
- **Reconstruir el contado** (`S = F − base`): necesita la previsión de dividendos de cada contrato, que no tenemos con fuente.
- **Medir la base implícita de cada contrato** con el diferencial entre el contrato cercano y el siguiente: es más exacto (incluye la prima de financiación de los futuros y los errores en la previsión de dividendos), pero necesita bajar el segundo contrato (`c.1`) de Databento, con su costo. Se puede hacer después si la diferencia con la tasa oficial importa para A.

**Oro (XAUUSD, GC):** el mismo razonamiento aplica. El precio de GC trae el contango (≈ tasa) y la financiación de IG para el oro se arma con la base de los futuros (ADR 0013, `commodity_basis`). Hay que revisarlo en el Issue del oro antes de simular tenencias de varios días.

## 7. Otros hallazgos

- **Posible error de datos en ES (dentro del fuera de muestra, sin mirar resultados):** al contar los cambios de contrato apareció que, del 17 al 19-11-2026, el continuo `ES.c.0` alterna entre dos `contract_id` (10252 y 42) a las 14:30–14:33 UTC, minuto a minuto. Un continuo por calendario no debería volver a un contrato anterior. Conviene revisarlo en un Issue de datos (podría partir velas de 15m o marcar vencimientos falsos). No se miró ningún precio ni resultado de ese tramo.

## Límites

- El sesgo se calculó fuera del motor, con la misma fórmula, sobre las operaciones volcadas. El PR de código lo tiene que reproducir dentro del motor, con tests.
- La tasa de la base se aproximó con EFFR→SOFR. La base real de los futuros incluye además una prima de financiación y errores en la previsión de dividendos (§3 lo confirma solo en orden de magnitud).
- Corregir cambia levemente qué operaciones se abren (capital, pérdida diaria, kill switch). No se volvió a correr con la corrección: el orden de magnitud (≈ 0,0015 R por operación) hace que el efecto de camino sea despreciable.
- El fuera de muestra (desde el 01-11-2025) no se usó en ningún cálculo de resultados.
