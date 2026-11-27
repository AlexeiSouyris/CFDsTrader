# Diagnóstico: dónde pierde y dónde gana la estrategia en US500, US100 y US30 (futuros, 2011–2025)

Issue #456 · 24-11-2026 · **solo estudio, en lectura**: no cambia ningún parámetro, ni `config/`, ni usa el fuera de muestra. Presenta hallazgos e hipótesis; **decide el dueño**. Es el mismo diagnóstico que #323 hizo para el DE40 (`2026-11-diagnostico-de40-futuros.md`), ahora con los futuros de EE. UU. y casi 15 años.

> **Advertencia de sobreajuste.** Son tres instrumentos, 12 cortes y entre 2 y 15 grupos por corte: unas **300 comparaciones**. Con IC del 95 %, se esperan unas 15 que "salgan significativas" por puro azar. Todo es en muestra, sobre el mismo período que después se usaría para evaluar un cambio. Ninguna diferencia de este informe justifica sola cambiar un parámetro: son **hipótesis**. Cualquiera que se quiera probar tiene que pre-registrarse (qué se cambia y qué resultado la confirmaría) y pasar por el walk-forward de la Fase 5 y, recién al final, por el fuera de muestra, que sigue intacto. Los resultados son pasado y no prometen ningún resultado futuro.

## Conclusión

1. **Pierden los tres, en casi todos los cortes.** Juntando 59 bloques trimestrales de 2011-02 a 2025-11:

   | Índice | Op. | Gana (IC 95 %) | R medio neto (IC 95 %) | Retorno medio por bloque | Mediana | Bloques positivos |
   |---|---|---|---|---|---|---|
   | US500 | 2988 | 32,1 % (30,5–33,8) | −0,19 (−0,24 a −0,15) | −10,4 % | −13,0 % | 9 de 59 |
   | US100 | 3080 | 34,7 % (33,0–36,4) | −0,12 (−0,16 a −0,07) | −6,8 % | −6,4 % | 12 de 59 |
   | US30 | 3177 | 33,3 % (31,7–34,9) | −0,16 (−0,20 a −0,11) | −8,5 % | −11,0 % | 9 de 59 |

   Coincide con el walk-forward de #405 (US500 −8 %, US30 −7 % por ventana, 2021–2025), ahora con 15 años.

2. **Antes de costos no hay ventaja, y los costos hacen la pérdida.** A precio medio, sin spread, slippage ni financiación, el resultado es casi cero: unos −0,02 R por operación en US500, +0,01 R en US100 y −0,05 R en US30. Los costos suman 0,18 R, 0,13 R y 0,10 R por operación. En US500 y US100 la pérdida neta es casi toda costos; en US30, un tercio es pérdida antes de costos.

3. **El peso de los costos cae con los años, porque el spread está fijo en puntos.** De 2011 a 2019 el índice cotizaba más bajo y el ATR en puntos era más chico, así que el mismo spread de config (0,9, 1,6 y 3,7 puntos) es una parte mayor del stop. Desde 2020:
   - US100 queda **cerca de cero neto** (−0,01 R por operación; 2021, 2022 y 2024 positivos);
   - US500 y US30 siguen perdiendo unos −0,11 R por operación.

   El mismo efecto aparece en el tercio bajo del ATR de 15m, que es el peor en los tres índices. **Ojo:** el spread histórico de IG no está medido (se usa el de config para todo el período, #267), así que la parte de los años viejos es la más incierta.

4. **El stop manda, como en el DE40.** El 58–60 % de las operaciones termina en el stop, a −1,05 a −1,09 R. Llega a TP1 un 24 %, que deja +1,9 R de media, y pasado TP1 ninguna pierde. La mitad de las perdedoras llegó a unos +0,4 R a favor y una de cada cuatro a +0,8 R, sin tocar TP1 (2 R).

5. **Lo que se repite en los tres índices** (los IC se superponen en casi todo):
   - **13:00 UTC**, antes de la apertura del contado (13:30 UTC en verano), es de las peores horas en US500 (−0,35 R) y US100 (−0,28 R). Las **17:00 UTC** pierden en los tres. Los StopTooFar se juntan a las 14:00 UTC (28–32 %).
   - **Lunes y martes** son los peores días; **el viernes**, el menos malo (gana el 41–43 %), pero con el payoff más bajo (1,15–1,26). Es el mismo dibujo que en el DE40: el cierre forzado del fin de semana corta las ganadoras (hipótesis sin verificar).
   - El **tercio bajo del ATR de 15m** (descriptivo) es el peor en los tres: −0,35, −0,24 y −0,22 R.
6. **Lo que NO se repite** (y por eso no sirve para decidir):
   - "Patrón de velas + rechazo en nivel", el peor gatillo del DE40, es de los **mejores** en US500 (−0,08 R) y US100 (−0,03 R), y de los peores en US30 (−0,20 R).
   - "Rechazo en nivel" solo es el peor gatillo con muestra grande en US500 (−0,32 R, IC −0,41 a −0,23, 736 op.), pero en US100 queda en −0,13 R.
   - La distancia del stop de 2,5–3 ATR es la mejor en US500 y US100 (como en el DE40), pero no en US30.
   - El tercio oficial de volatilidad no da un patrón común.
7. **La tendencia diaria casi no separa.** En US30 operar a favor de la tendencia diaria pierde la mitad (−0,10 R) que en contra (−0,21 R); en US100 es −0,09 frente a −0,13 R; en US500 no hay diferencia. La tendencia de 1h no se puede usar como corte: el 95 % de las operaciones ya va a favor, porque el puntaje lo pide.
8. **La financiación no cambia nada.** Con EFFR→SOFR en lugar de EONIA→€STR, el R medio se mueve 0,01 R como mucho y las conclusiones son las mismas. El swap es el 5–10 % de los costos.

**Para el dueño:** el problema de US500, US100 y US30 no es un horario ni un gatillo, sino que **no hay ventaja antes de costos** y los costos se comen 0,10–0,18 R por operación. Ningún filtro de este informe da vuelta el signo con una muestra grande y consistente en los tres índices. Las hipótesis de abajo son candidatas para un estudio pre-registrado, no cambios.

## Datos y método

- **Herramienta:** el ejemplo `diagnostico` (`engine/crates/backtest/examples/diagnostico.rs`, #323), ampliado en este PR:
  - acepta los flags de estudio compartidos (`examples/common/study.rs`, #405, #421): `--escenario-financiacion` pone el tipo de referencia **solo en memoria**, porque IG todavía no lo confirmó (`config/instruments.toml` lo tiene `pending`);
  - `--tramos`: en vez de un solo recorrido de 15 años, junta los **bloques trimestrales antes del fuera de muestra** (`research_blocks`, con `config/data_split.toml`), cada uno desde el capital inicial y con los 30 días de calentamiento de `config/evaluation.toml`, igual que las ventanas de prueba del walk-forward. La misma guardia (`research_period`) no deja que ningún bloque llegue al fuera de muestra;
  - cortes nuevos: año, tendencia de 1h y diaria respecto de la dirección, y en cada corte el P/L a precio medio, los costos y el neto, con un IC del R medio.
- **Por qué bloques:** un recorrido único de 2011 a 2025 se probó primero y **no sirve**: en US500 el capital cae de 10 000 a 280 hacia 2018 y desde ahí casi todas las señales se rechazan por tamaño mínimo (4436 `BelowMinDealSize`), así que los años siguientes no tienen operaciones. Con bloques, cada trimestre opera con el mismo riesgo (1 R ≈ 100 €).
- **Datos:** velas de futuros de Databento (ES, NQ y YM continuos, `--fuente futuros`), completas desde el 02-01-2011 hasta el 22-11-2026. Primer bloque 01-02-2011, último 01-08-2025 → 01-11-2025. **El fuera de muestra (desde el 01-11-2025) no se usó.** Cierre antes de cada vencimiento, spread de config (0,9 puntos US500, 1,6 US100, 3,7 US30; filtro de spread §7.3 sin aplicar, #267), sin calendario de noticias ni interpretaciones en el período (§7.2 y §9 sin efecto).
- **Financiación:**
  - **principal:** EONIA→€STR empalmada (`docs/analysis/2026-11-eur-eonia-estr.csv`, BCE, #421);
  - **sensibilidad:** EFFR→SOFR empalmada (`docs/analysis/2026-11-usd-effr-sofr.csv`, NY Fed, #421).

  Cada valor cuenta desde las 13:00 UTC de dos días hábiles después de su fecha: sin look-ahead. La tasa administrativa y el resto de los costos son los del catálogo.
- **Base real en solo lectura:** cada corrida usó una conexión con `default_transaction_read_only=on`. No se escribió nada en `backtest_runs` ni en ninguna tabla.
- **Montos** en la moneda de la cuenta (EUR), solo agregados. **R** = tamaño × distancia del stop desde la entrada ejecutada. "Sin costos" = P/L a precio medio; "Costos" = spread + slippage + comisión + financiación.
- **Intervalos:** las tasas llevan el IC 95 % de Wilson y el R medio un IC normal (media ± z·s/√n), con z de `config/probability.toml`. Menos de 30 operaciones = **muestra chica**.
- **Comandos** (uno por índice y escenario):

  ```text
  cargo run --release -p cfds-backtest --example diagnostico -- \
      US500 2011-01-01T00:00:00Z 2026-11-23T00:00:00Z 10000 --fuente futuros --tramos \
      --escenario-financiacion EONIA_ESTR=../docs/analysis/2026-11-eur-eonia-estr.csv

  cargo run --release -p cfds-backtest --example diagnostico -- \
      US500 2011-01-01T00:00:00Z 2026-11-23T00:00:00Z 10000 --fuente futuros --tramos \
      --escenario-financiacion EFFR_SOFR=../docs/analysis/2026-11-usd-effr-sofr.csv
  ```

  La fecha final llega hasta la última vela para que el fuera de muestra se calcule igual que en el walk-forward. Los bloques terminan antes de él.

## Costos: antes y después

Escenario principal (EONIA→€STR). R aproximado con 1 R ≈ 100 €.

| Índice | P/L sin costos | Spread | Slippage | Financiación | Costos | Neto | Costo por op. |
|---|---|---|---|---|---|---|---|
| US500 | −5346 | 43 528 | 9672 | 2961 | 56 162 | −61 509 | 0,18 R |
| US100 | +3422 | 31 428 | 10 214 | 2413 | 44 055 | −40 633 | 0,13 R |
| US30 | −15 011 | 25 290 | 6425 | 3466 | 35 182 | −50 193 | 0,10 R |

No hay comisión. Control: spread + slippage coincide exacto con P/L a precio medio − P/L bruto en los tres.

### Antes y desde 2020

| Índice | Período | Op. | Sin costos | Costos | Neto | Neto por op. |
|---|---|---|---|---|---|---|
| US500 | 2011–2019 | 1644 | −4454 | 41 668 | −46 124 | ≈ −0,28 R |
| US500 | 2020–2025 | 1344 | −890 | 14 489 | −15 377 | ≈ −0,11 R |
| US100 | 2011–2019 | 1739 | −2622 | 37 111 | −39 731 | ≈ −0,23 R |
| US100 | 2020–2025 | 1341 | +6045 | 6939 | −896 | ≈ −0,01 R |
| US30 | 2011–2019 | 1869 | −9764 | 26 553 | −36 318 | ≈ −0,19 R |
| US30 | 2020–2025 | 1308 | −5243 | 8621 | −13 866 | ≈ −0,11 R |

Son sumas de las filas por año (por redondeo pueden diferir en unas unidades de los totales). Los costos por operación bajan a menos de la mitad desde 2020. Una parte puede ser real (el índice más alto hace que un spread en puntos pese menos) y otra un artefacto: el spread de IG de 2011 no se conoce.

### Sensibilidad de la financiación

| Índice | Escenario | Op. | Financiación | Neto | R medio | Retorno medio por bloque |
|---|---|---|---|---|---|---|
| US500 | EONIA→€STR | 2988 | 2961 | −61 509 | −0,19 | −10,4 % |
| US500 | EFFR→SOFR | 2974 | 3286 | −62 215 | −0,20 | −10,5 % |
| US100 | EONIA→€STR | 3080 | 2413 | −40 633 | −0,12 | −6,8 % |
| US100 | EFFR→SOFR | 3080 | 2720 | −40 929 | −0,12 | −6,9 % |
| US30 | EONIA→€STR | 3177 | 3466 | −50 193 | −0,16 | −8,5 % |
| US30 | EFFR→SOFR | 3186 | 3748 | −49 483 | −0,15 | −8,3 % |

La cantidad de operaciones cambia un poco por efectos de camino (el kill switch o la pérdida diaria saltan en otro momento), igual que en #405.

## Cortes (escenario principal)

Columnas: operaciones, % que gana (IC 95 %), P/L sin costos, costos, neto, R medio neto (IC 95 %) y payoff.

### Año

| Año | US500 op. | US500 sin costos | US500 neto | US500 R (IC) | US100 sin costos | US100 neto | US100 R (IC) | US30 sin costos | US30 neto | US30 R (IC) |
|---|---|---|---|---|---|---|---|---|---|---|
| 2011 | 198 | +5263 | −346 | −0,00 (−0,19 a +0,19) | +934 | −3642 | −0,18 (−0,38 a +0,00) | −626 | −3474 | −0,15 (−0,33 a +0,01) |
| 2012 | 174 | −786 | −6431 | −0,34 (−0,52 a −0,16) | +1333 | −4547 | −0,22 (−0,40 a −0,04) | −1641 | −4882 | −0,25 (−0,43 a −0,08) |
| 2013 | 171 | −477 | −6164 | −0,35 (−0,52 a −0,17) | −2447 | −8526 | −0,44 (−0,60 a −0,27) | +157 | −4334 | −0,18 (−0,34 a −0,03) |
| 2014 | 141 | −2313 | −5981 | −0,39 (−0,59 a −0,19) | +138 | −3805 | −0,19 (−0,39 a +0,00) | −767 | −4167 | −0,20 (−0,39 a −0,02) |
| 2015 | 224 | +100 | −4722 | −0,20 (−0,37 a −0,04) | −874 | −4945 | −0,20 (−0,36 a −0,04) | −2749 | −5183 | −0,25 (−0,42 a −0,07) |
| 2016 | 183 | −525 | −5213 | −0,28 (−0,46 a −0,10) | +407 | −3309 | −0,16 (−0,35 a +0,01) | −1936 | −4619 | −0,24 (−0,41 a −0,07) |
| 2017 | 167 | −3023 | −7759 | −0,46 (−0,62 a −0,29) | −491 | −4655 | −0,23 (−0,41 a −0,06) | −373 | −3739 | −0,20 (−0,37 a −0,03) |
| 2018 | 204 | −255 | −3566 | −0,16 (−0,34 a +0,01) | −1192 | −3369 | −0,15 (−0,33 a +0,01) | +340 | −1483 | −0,06 (−0,25 a +0,11) |
| 2019 | 182 | −2438 | −5942 | −0,30 (−0,48 a −0,12) | −430 | −2933 | −0,14 (−0,32 a +0,04) | −2169 | −4437 | −0,18 (−0,34 a −0,03) |
| 2020 | 235 | −261 | −2998 | −0,12 (−0,29 a +0,04) | +140 | −1194 | −0,04 (−0,22 a +0,12) | −464 | −1897 | −0,08 (−0,27 a +0,11) |
| 2021 | 192 | +2503 | −170 | −0,01 (−0,20 a +0,16) | +2174 | +827 | +0,03 (−0,13 a +0,20) | +500 | −1211 | −0,07 (−0,24 a +0,09) |
| 2022 | 250 | +1270 | −652 | −0,02 (−0,19 a +0,15) | +2676 | +1878 | +0,08 (−0,10 a +0,27) | −1467 | −2719 | −0,10 (−0,25 a +0,05) |
| 2023 | 246 | −1764 | −4665 | −0,18 (−0,34 a −0,02) | −1670 | −3122 | −0,11 (−0,28 a +0,04) | −691 | −2400 | −0,11 (−0,28 a +0,05) |
| 2024 | 250 | +376 | −2412 | −0,08 (−0,25 a +0,08) | +4163 | +2886 | +0,12 (−0,05 a +0,31) | −833 | −2439 | −0,11 (−0,27 a +0,04) |
| 2025 | 171 | −3014 | −4480 | −0,25 (−0,43 a −0,08) | −1438 | −2171 | −0,11 (−0,30 a +0,06) | −2288 | −3200 | −0,20 (−0,40 a −0,01) |

2025 llega solo hasta octubre. Ningún año es positivo con un IC que excluya el cero.

### Hora de entrada (UTC)

El contado de Nueva York abre a las 13:30 UTC en verano y a las 14:30 UTC en invierno.

| Hora | US500 op. | US500 gana | US500 R (IC) | US100 op. | US100 gana | US100 R (IC) | US30 op. | US30 gana | US30 R (IC) |
|---|---|---|---|---|---|---|---|---|---|
| 13:00 | 238 | 26,4 % | −0,35 (−0,51 a −0,19) | 211 | 26,5 % | −0,28 (−0,46 a −0,09) | 238 | 31,5 % | −0,15 (−0,33 a +0,01) |
| 14:00 | 630 | 32,3 % | −0,19 (−0,30 a −0,09) | 681 | 36,1 % | −0,10 (−0,20 a −0,00) | 670 | 31,4 % | −0,15 (−0,25 a −0,05) |
| 15:00 | 629 | 31,9 % | −0,20 (−0,30 a −0,10) | 673 | 35,0 % | −0,08 (−0,18 a +0,01) | 609 | 31,6 % | −0,19 (−0,29 a −0,09) |
| 16:00 | 420 | 36,6 % | −0,06 (−0,19 a +0,06) | 458 | 35,8 % | −0,08 (−0,20 a +0,03) | 442 | 35,5 % | −0,09 (−0,21 a +0,03) |
| 17:00 | 370 | 29,4 % | −0,29 (−0,41 a −0,17) | 328 | 32,6 % | −0,18 (−0,32 a −0,04) | 375 | 31,4 % | −0,24 (−0,36 a −0,11) |
| 18:00 | 302 | 32,4 % | −0,13 (−0,28 a +0,01) | 353 | 36,5 % | −0,09 (−0,22 a +0,04) | 394 | 36,8 % | −0,13 (−0,25 a −0,00) |
| 19:00 | 301 | 33,8 % | −0,16 (−0,29 a −0,03) | 281 | 34,5 % | −0,14 (−0,29 a −0,00) | 330 | 36,9 % | −0,10 (−0,24 a +0,02) |
| 20:00 | 98 | 30,6 % | −0,21 (−0,46 a +0,03) | 95 | 35,7 % | −0,13 (−0,38 a +0,10) | 119 | 31,9 % | −0,21 (−0,41 a −0,02) |

StopTooFar por hora de la decisión: 13:00 UTC 14–19 %, 14:00 UTC 28–32 %, 15:00 UTC 19–21 %; el resto, menos del 11 % cada una.

### Gatillos (grupos con 30 operaciones o más en los tres)

| Gatillos | US500 op. | US500 R (IC) | US100 op. | US100 R (IC) | US30 op. | US30 R (IC) |
|---|---|---|---|---|---|---|
| rechazo en nivel | 736 | −0,32 (−0,41 a −0,23) | 777 | −0,13 (−0,23 a −0,04) | 880 | −0,19 (−0,27 a −0,10) |
| patrón de velas | 750 | −0,18 (−0,27 a −0,09) | 684 | −0,15 (−0,25 a −0,06) | 742 | −0,08 (−0,18 a +0,01) |
| patrón de velas + rechazo en nivel | 521 | −0,08 (−0,19 a +0,03) | 434 | −0,03 (−0,15 a +0,09) | 537 | −0,20 (−0,31 a −0,10) |
| rechazo en nivel + sweep | 466 | −0,23 (−0,35 a −0,12) | 568 | −0,10 (−0,21 a +0,00) | 469 | −0,12 (−0,24 a −0,00) |
| patrón de velas + rechazo en nivel + sweep | 162 | −0,23 (−0,42 a −0,04) | 212 | −0,16 (−0,33 a +0,00) | 199 | −0,20 (−0,37 a −0,04) |
| patrón de velas + sweep | 85 | −0,13 (−0,42 a +0,15) | 88 | −0,18 (−0,43 a +0,07) | 65 | −0,10 (−0,42 a +0,22) |
| doble techo/suelo + rechazo en nivel | 83 | +0,07 (−0,19 a +0,34) | 118 | −0,02 (−0,24 a +0,18) | 89 | −0,10 (−0,34 a +0,13) |
| sweep | 62 | −0,31 (−0,63 a +0,00) | 67 | −0,40 (−0,68 a −0,12) | 71 | −0,37 (−0,63 a −0,10) |
| doble techo/suelo | 65 | +0,14 (−0,13 a +0,42) | 54 | +0,03 (−0,28 a +0,35) | 44 | −0,12 (−0,44 a +0,19) |

- "Sweep" solo es malo en los tres, con muestras de 60–70.
- Las combinaciones con doble techo/suelo andan algo mejor en US500 y US100, como en el DE40, pero con IC que cruzan el cero.

### Dirección

| Dirección | US500 op. | US500 sin costos | US500 R (IC) | US100 op. | US100 sin costos | US100 R (IC) | US30 op. | US30 sin costos | US30 R (IC) |
|---|---|---|---|---|---|---|---|---|---|
| largo | 1686 | −1515 | −0,20 (−0,26 a −0,14) | 1807 | +9071 | −0,09 (−0,15 a −0,03) | 1753 | −376 | −0,12 (−0,18 a −0,06) |
| corto | 1302 | −3830 | −0,18 (−0,25 a −0,11) | 1273 | −5649 | −0,16 (−0,23 a −0,08) | 1424 | −14 634 | −0,20 (−0,26 a −0,13) |

En US100 y US30 los cortos explican casi toda la pérdida antes de costos. En un período en que los índices subieron mucho, es lo esperable, y no dice que los cortos vayan a perder en el futuro.

### Tendencia respecto de la dirección

| Tendencia | US500 op. | US500 R (IC) | US100 op. | US100 R (IC) | US30 op. | US30 R (IC) |
|---|---|---|---|---|---|---|
| 1h a favor | 2831 | −0,19 (−0,24 a −0,14) | 2879 | −0,13 (−0,18 a −0,09) | 2991 | −0,16 (−0,21 a −0,12) |
| 1h neutral | 156 | −0,24 (−0,44 a −0,05) | 199 | +0,11 (−0,07 a +0,30) | 182 | −0,06 (−0,24 a +0,11) |
| diaria a favor | 1431 | −0,19 (−0,26 a −0,13) | 1536 | −0,09 (−0,15 a −0,02) | 1450 | −0,10 (−0,17 a −0,04) |
| diaria neutral | 551 | −0,19 (−0,30 a −0,09) | 515 | −0,16 (−0,27 a −0,05) | 526 | −0,17 (−0,28 a −0,06) |
| diaria en contra | 983 | −0,20 (−0,28 a −0,11) | 1016 | −0,13 (−0,21 a −0,05) | 1168 | −0,21 (−0,28 a −0,13) |

- **1h:** es la tendencia del puntaje (spec §4). En contra hay 1 a 4 operaciones, así que el corte casi no separa.
- **Diaria:** la misma regla de `[trend]` (SMA de 50 y pendiente a 5) sobre velas diarias **cerradas** antes de la señal. Es descriptiva: la estrategia no la usa, y la regla está pensada para 1h. Entre 13 y 33 operaciones por índice no tienen dato (calentamiento).

### Día de la semana (UTC)

| Día | US500 gana | US500 R (IC) | US500 payoff | US100 gana | US100 R (IC) | US100 payoff | US30 gana | US30 R (IC) | US30 payoff |
|---|---|---|---|---|---|---|---|---|---|
| lunes | 27,7 % | −0,26 (−0,37 a −0,15) | 1,63 | 32,2 % | −0,14 (−0,25 a −0,03) | 1,62 | 28,5 % | −0,26 (−0,36 a −0,16) | 1,60 |
| martes | 28,5 % | −0,27 (−0,37 a −0,16) | 1,59 | 31,3 % | −0,20 (−0,30 a −0,10) | 1,53 | 31,3 % | −0,20 (−0,30 a −0,09) | 1,57 |
| miércoles | 32,2 % | −0,16 (−0,26 a −0,06) | 1,57 | 32,2 % | −0,15 (−0,25 a −0,05) | 1,62 | 32,0 % | −0,15 (−0,24 a −0,06) | 1,63 |
| jueves | 31,4 % | −0,19 (−0,29 a −0,09) | 1,57 | 36,7 % | −0,02 (−0,12 a +0,08) | 1,65 | 32,8 % | −0,14 (−0,24 a −0,05) | 1,58 |
| viernes | 41,3 % | −0,09 (−0,19 a +0,00) | 1,17 | 41,8 % | −0,08 (−0,18 a +0,01) | 1,15 | 42,7 % | −0,03 (−0,12 a +0,06) | 1,26 |

Entre 518 y 748 operaciones por día e índice.

### Motivo de salida

| Salidas | US500 op. | US500 R | US100 op. | US100 R | US30 op. | US30 R |
|---|---|---|---|---|---|---|
| stop | 1768 (59 %) | −1,09 | 1799 (58 %) | −1,07 | 1893 (60 %) | −1,05 |
| TP1 → trailing | 630 | +1,88 | 688 | +1,93 | 671 | +1,91 |
| TP1 → break-even | 36 | +0,89 | 35 | +0,93 | 45 | +0,92 |
| TP1 → cierre forzado | 33 | +2,25 | 41 | +2,25 | 42 | +2,33 |
| cierre forzado | 218 | +0,38 | 223 | +0,41 | 211 | +0,42 |
| por tiempo | 294 | −0,13 | 287 | −0,01 | 308 | −0,07 |
| fin de datos (fin del bloque) | 9 | +0,4 a +2,4 | 7 | +0,1 a +2,0 | 7 | +0,38 |

"Fin de datos" son operaciones abiertas al terminar un bloque: un artefacto de cortar en trimestres, con pocas operaciones.

### Distancia del stop (en ATR de 15m al decidir; el riesgo rechaza más de 3)

| Distancia | US500 op. | US500 R (IC) | US100 op. | US100 R (IC) | US30 op. | US30 R (IC) |
|---|---|---|---|---|---|---|
| menos de 1 | 230 | −0,33 (−0,51 a −0,15) | 268 | −0,20 (−0,37 a −0,03) | 261 | −0,21 (−0,38 a −0,05) |
| 1 a 1,5 | 817 | −0,20 (−0,29 a −0,11) | 855 | −0,19 (−0,28 a −0,10) | 888 | −0,21 (−0,30 a −0,12) |
| 1,5 a 2 | 828 | −0,21 (−0,29 a −0,12) | 887 | −0,07 (−0,16 a +0,00) | 861 | −0,12 (−0,21 a −0,03) |
| 2 a 2,5 | 610 | −0,21 (−0,31 a −0,12) | 592 | −0,11 (−0,21 a −0,01) | 650 | −0,09 (−0,19 a −0,00) |
| 2,5 a 3 | 503 | −0,07 (−0,17 a +0,03) | 478 | −0,04 (−0,15 a +0,06) | 517 | −0,17 (−0,27 a −0,07) |

Con el riesgo fijo en el 1 %, un stop más corto hace que el spread en puntos sea una parte mayor de R: es el mismo mecanismo de los costos.

### Volatilidad

**Tercio oficial** (`daily_vol_terciles`, 3 años de historia; hay tercios desde 2014-02, así que el 18–20 % de las operaciones queda sin dato):

| Tercio | US500 op. | US500 R (IC) | US100 op. | US100 R (IC) | US30 op. | US30 R (IC) |
|---|---|---|---|---|---|---|
| bajo | 833 | −0,21 (−0,30 a −0,12) | 872 | −0,07 (−0,16 a +0,01) | 825 | −0,11 (−0,20 a −0,02) |
| medio | 751 | −0,25 (−0,34 a −0,16) | 764 | −0,07 (−0,16 a +0,02) | 787 | −0,20 (−0,29 a −0,11) |
| alto | 857 | −0,11 (−0,20 a −0,02) | 890 | −0,11 (−0,20 a −0,02) | 917 | −0,12 (−0,21 a −0,04) |

**Tercio del ATR de 15m al decidir** (descriptivo: los cortes salen de toda la muestra, **no sirve para decidir en vivo**):

| Tercio | US500 R (IC) | US500 costos | US100 R (IC) | US100 costos | US30 R (IC) | US30 costos |
|---|---|---|---|---|---|---|
| bajo | −0,35 (−0,42 a −0,27) | 30 600 | −0,24 (−0,32 a −0,17) | 28 337 | −0,22 (−0,30 a −0,15) | 18 785 |
| medio | −0,14 (−0,22 a −0,06) | 16 882 | −0,07 (−0,15 a +0,00) | 11 157 | −0,13 (−0,21 a −0,05) | 10 585 |
| alto | −0,09 (−0,17 a −0,01) | 8679 | −0,04 (−0,12 a +0,03) | 4561 | −0,11 (−0,19 a −0,03) | 5811 |

El tercio bajo del ATR paga de 3 a 6 veces más costos que el alto. Es lo mismo que se vio por año y por distancia del stop.

## MAE y MFE en R

| Medida | Índice | p10 | p25 | Mediana | p75 | p90 |
|---|---|---|---|---|---|---|
| MAE (todas) | US500 / US100 / US30 | 0,28 / 0,26 / 0,24 | 0,58 / 0,56 / 0,62 | 1,04 / 1,04 / 1,05 | 1,23 / 1,26 / 1,26 | 1,56 / 1,57 / 1,56 |
| MFE (todas) | US500 / US100 / US30 | 0,02 / 0,03 / 0,04 | 0,21 / 0,26 / 0,24 | 0,71 / 0,75 / 0,70 | 1,84 / 2,00 / 1,88 | 2,81 / 2,86 / 2,85 |
| MFE de las perdedoras | US500 / US100 / US30 | −0,01 / −0,00 / 0,00 | 0,10 / 0,12 / 0,13 | 0,37 / 0,40 / 0,39 | 0,80 / 0,81 / 0,80 | 1,31 / 1,32 / 1,32 |

Se miden entre la vela de entrada y la de la última salida, con velas de 15m: son cotas, no el recorrido exacto. TP1 está a 2 R.

## Rechazos del riesgo

| Motivo | US500 | US100 | US30 |
|---|---|---|---|
| StopTooFar (stop a más de 3 ATR) | 1919 | 2040 | 2307 |
| DailyLossLimit | 69 | 76 | 65 |
| DrawdownLimit (activa el kill switch) | 38 | 34 | 36 |
| BelowMinDealSize | 1 | 7 | 2 |

Eventos del kill switch (activaciones y reactivaciones, que en backtest vuelven a los 20 días hábiles, #277): 65 en US500, 57 en US100 y 50 en US30.

## Hipótesis para validar después (no son recomendaciones)

Cada una necesita un pre-registro y el walk-forward antes de tocar la config. Ninguna está probada fuera de esta muestra.

1. **Costos relativos al stop.** Es lo más consistente de este informe: año, distancia del stop y tercio del ATR cuentan lo mismo. Hay dos caminos:
   - medir el spread real de IG en US500, US100 y US30 (hoy es el de config);
   - estudiar un piso del stop en múltiplos del spread, o no operar cuando el ATR de 15m es bajo frente al spread. Un umbral así es fácil de sobreajustar: tiene que definirse antes de mirar el resultado.
2. **No abrir a las 13:00 UTC** (antes de la apertura del contado en verano). Es negativo en los tres índices, pero solo en US500 y US100 el IC excluye el cero, con ~220 operaciones por índice. La config es `config/trading_window.toml`.
3. **El viernes.** Acierta más y tiene el payoff más bajo, en los tres y como en el DE40. Habría que estudiar el cierre forzado del fin de semana (¿corta ganadoras?) antes de pensar en un filtro.
4. **"Sweep" solo** pierde en los tres, pero con muestras de 60–70 por índice. El peso de cada gatillo está en `config/strategy.toml` `score`.
5. **Lo que no conviene probar** por inconsistente: el peso de "patrón de velas + rechazo en nivel" (contradice al DE40), el tercio oficial de la volatilidad y la tendencia diaria.

## Límites

- **En muestra:** el mismo período que después se usaría para evaluar; ningún resultado de acá es fuera de muestra.
- **Comparaciones múltiples:** unas 300; los extremos se explican en parte por azar y los IC se superponen en casi todo.
- **Futuros, no el CFD:** las velas son de ES, NQ y YM. El spread es el de config para los 15 años, no uno medido, y el de 2011 seguramente era otro.
- **Financiación supuesta:** los dos escenarios son rotulados, porque IG no confirmó el tipo de referencia de estos contratos. Pesan poco.
- **Sin noticias:** no hay calendario ni interpretaciones en el período.
- **Bloques trimestrales:** cada uno arranca desde 10 000 €, así que no se ve el efecto de una racha larga sobre el capital (por eso el recorrido único no servía). Las operaciones abiertas al cerrar un bloque salen como "fin de datos".
- **Cortes descriptivos:** el tercio del ATR usa cortes de toda la muestra y la tendencia diaria no es parte de la estrategia.
- **MAE y MFE** salen de velas de 15m: son cotas.
