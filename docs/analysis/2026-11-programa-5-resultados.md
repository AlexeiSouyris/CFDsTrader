# Programa 5: resultados de H5.1 y H5.2 (FVG)

Issue #649 · corrido el 25-11-2026 por un agente de IA (Claude Code) · pre-registro congelado: `2026-11-programa-5-fvg-preregistro.md` (SHA-256 en `2026-11-programa-5-fvg-preregistro.md.sha256`, PR #653).

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real: es un estudio en simulación sobre datos históricos.

## Resumen simple

- **H5.1 (¿el FVG importa?): NO PASA.** Cuando el precio toca por primera vez un FVG de 15 minutos, la reacción de las 8 velas siguientes **no es mayor** que la de un precio cualquiera tocado en las mismas condiciones (el placebo). Es al revés: el FVG reacciona un poco **menos** que el placebo (−0,023 ATR por mes en promedio; p = 0,96 con el nivel de 0,025). Falla en los dos índices, en las dos mitades y también a 4 velas.
- **H5.2 (¿sirve como filtro?): NO PASA**, con los dos escenarios de financiación. Las operaciones de la estrategia en uso que coinciden con el primer toque de un FVG a favor **no ganan más** que las demás: pierden −0,169 R por operación contra −0,151 R del resto (p = 0,66). En US100 dan algo mejor y en US500 bastante peor; en 2013–2020 algo mejor y en 2021–2025 peor.
- **Descriptivo:** en velas de 1 hora el FVG también reacciona menos que el placebo (−0,063 ATR). La estrategia "solo con las operaciones F" pierde contra comprar y mantener en los dos índices.
- **Qué quiere decir:** con estas reglas, el primer toque de un FVG no mostró un rebote propio en 2013–2025, y no separa operaciones mejores de peores en la estrategia actual. Las dos hipótesis se cierran: no se prueban variantes sobre estos datos (§10 del pre-registro). El fuera de muestra (desde el 01-11-2025) **no se miró**.
- **Un error de código, corregido:** la primera corrida de H5.1 empezaba un mes antes (01-2013, 154 meses) en vez de 02-2013. Se corrigió y se volvió a correr; las dos corridas se informan (§4) y las dos dan **no pasa**.

## 1. Corridas

Todas con una conexión de solo lectura (`default_transaction_read_only=on`), velas hasta el 01-11-2025 (el fuera de muestra no se cargó), con el ejemplo `programa_5` (`engine/crates/backtest/examples/programa_5.rs`) commiteado antes de correr. Los hashes son los de los mismos commits después de rebasar sobre `main` (al correr eran `0f343c8` y `f5e57c5`; el código no cambió).

| # | Hora (UTC) | Comando | Commit | Qué es |
|---|---|---|---|---|
| 1 | 25-11-2026 14:23:35 | `programa_5 h51` | `62c32ce` | primera corrida de H5.1, **con un error de código** (§4); no vale |
| 2 | 25-11-2026 14:24:30 | `programa_5 h51` | `3f03e3c` | H5.1 corregida: la que vale |
| 3 | 25-11-2026 14:25:01 | `programa_5 h52 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv` | `3f03e3c` | H5.2, escenario EUR |
| 4 | 25-11-2026 14:25:19 | `programa_5 h52 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv` | `3f03e3c` | H5.2, escenario USD |

Antes de calcular reacciones, cada corrida imprimió las cantidades (§10 del pre-registro). Los eventos reales coinciden exactamente con el conteo previo (§7 y §13.2 del pre-registro).

## 2. H5.1: reacción al primer toque frente al placebo (15 m, vota)

**Eventos** (02-2013 → 10-2025, 153 meses):

| | US100 | US500 |
|---|---|---|
| Primeros toques en el período | 59.636 | 51.101 |
| Pierde por vencimiento (F1) | 462 | 363 |
| Pierde por llenar en el toque (F2) | 13.443 | 9.302 |
| Eventos reales | 45.731 | 41.436 |
| Sin ninguna candidata a placebo (salen) | 480 | 436 |
| **Con placebo** | **45.251** | **41.000** |
| Candidatas por evento (media) | 260 | 277 |

**Criterios** (`D_m` = media de `r_8` real − media de `r_8` de sus placebos, por mes; en ATR):

| # | Criterio | Resultado | ¿Cumple? |
|---|---|---|---|
| 1 | Media de D̄ > 0 con p < 0,025 (t con Newey-West de 3 rezagos) | media **−0,0233 ATR**, t = −1,80, **p = 0,963** (sin Newey-West: t = −1,53) | no |
| 2 | Media de D_m > 0 en cada índice | US100 −0,0143; US500 −0,0324 | no |
| 3 | Media de D̄ > 0 en las dos mitades | 2013–2020 −0,0113 (95 meses); 2021–2025 −0,0430 (58 meses) | no |
| 4 | Con `r_4`, media de D̄ > 0 | −0,0002 | no |
| 5 | ≥ 1.000 eventos con placebo por índice | 45.251 y 41.000 | sí |

**Veredicto H5.1: NO PASA** (falla 1, 2, 3 y 4).

**Test de suerte** (1.000 repeticiones del sorteo, solo reporte): la media real de `r_8` (+0,0071 ATR) queda en el **percentil 0** de las 1.000 medias de placebo (media +0,0198; mínimo +0,0084). Por índice: US100 percentil 5,5; US500 percentil 0,1. Es decir, el toque real reacciona **menos** que casi cualquier sorteo de placebos.

### Descriptivo (15 m, sin voto)

| | US100 real | US100 placebo | US500 real | US500 placebo |
|---|---|---|---|---|
| `r_8` media (mediana) | +0,0072 (+0,0089) | +0,0205 (+0,0503) | +0,0069 (0,0000) | +0,0359 (+0,0398) |
| `r_8` rango intercuartílico | −1,04 a +1,06 | −1,16 a +1,22 | −1,03 a +1,05 | −1,12 a +1,19 |
| `r_4` media | +0,0159 | +0,0118 | +0,0137 | +0,0165 |
| `r_8` > 0 (IC 95 %) | 50,0 % [49,6; 50,5] | 50,8 % [50,6; 51,0] | 49,0 % [48,5; 49,5] | 50,2 % [49,9; 50,4] |

Real − placebo (`r_8`, en ATR, media por evento) por corte:

| Corte | US100 | US500 |
|---|---|---|
| Tamaño: tercil chico / medio / grande | −0,013 / −0,038 / +0,011 | −0,015 / −0,048 / −0,024 |
| Dirección: alcista / bajista | −0,032 / +0,008 | −0,043 / −0,014 |
| Franja: contado / fuera | +0,006 / −0,021 | −0,011 / −0,037 |
| Espera: 1 / 2–5 / 6–20 / > 20 velas | −0,008 / −0,010 / +0,029 / −0,094 | −0,039 / +0,014 / −0,088 / −0,010 |
| Con ATR[t − 1] en vez de ATR[i] (el real) | −0,006 | −0,029 |

Por año, la diferencia cambia de signo de un año a otro (US100: 6 años positivos de 13; US500: 4 de 13), sin ninguna tendencia.

**Costos** (H5.1 no opera, los costos no votan): la diferencia real − placebo en puntos es de **+0,01 puntos en contado y −0,55 fuera** en US100 (ida y vuelta del CFD: 2,12 y 3,20 puntos) y de **−0,08 y −0,19** en US500 (1,10 y 1,94). Aun donde es positiva (US100 en contado), es unas 150 veces más chica que el costo de ida y vuelta.

### Descriptivo en velas de 1 hora (sin voto)

10.652 eventos con placebo en US100 y 10.161 en US500. La media de D̄ es **−0,063 ATR** (t con Newey-West −2,17): el FVG de 1 hora reacciona **menos** que el placebo, en los dos índices (−0,069 y −0,057), en las dos mitades (−0,082 y −0,031) y a 4 velas (−0,024). Los FVG grandes son los que peor quedan (tercil grande: −0,17 y −0,13).

## 3. H5.2: operaciones de la estrategia en uso con FVG a favor (vota)

Las operaciones de la estrategia en uso (`config/strategy.toml` y `config/risk.toml` sin cambios) en las 51 ventanas de validación, cada una desde 10.000 € (como el `diagnostico` de #456). **F** = la vela de la señal es el primer toque de un FVG de 15 m de la misma dirección (§5.2).

| | US100 | US500 |
|---|---|---|
| Operaciones EUR (F / R) | 2.692 (**577** / 2.115) | 2.606 (**566** / 2.040) |
| Operaciones USD (F / R) | 2.690 (**577** / 2.113) | 2.598 (**565** / 2.033) |

**Criterios** (R neto por operación; F y R juntan US100 y US500):

| # | Criterio | EUR | USD | ¿Cumple? |
|---|---|---|---|---|
| 1 | F − R > 0 con p < 0,025 (Welch) | F −0,1689 R − R −0,1506 R = **−0,0183 R**; t = −0,42; **p = 0,663** | **−0,0179 R**; t = −0,41; **p = 0,659** | no |
| 2 | F − R > 0 en cada índice | US100 **+0,077**; US500 **−0,115** | US100 +0,076; US500 −0,113 | no (US500) |
| 3 | F − R > 0 en las dos mitades | 2013–2020 +0,013; 2021–2025 **−0,065** | +0,013; −0,064 | no (2021–2025) |
| 4 | Con costos × 1,5, F − R > 0 | −0,023 | −0,023 | no |
| 5 | ≥ 100 operaciones F por índice | 577 y 566 | 577 y 565 | sí |

**Veredicto H5.2: NO PASA** en los dos escenarios (falla 1, 2, 3 y 4).

**Test de suerte** (1.000 subconjuntos al azar del mismo tamaño que F en cada índice, solo reporte): el R neto medio de F queda en el **percentil 31,7 (EUR) y 35,9 (USD)**: un grupo de operaciones elegido al azar suele dar lo mismo o mejor.

### Descriptivo (sin voto, escenario EUR; USD casi igual)

| | F | R |
|---|---|---|
| Operaciones | 1.143 | 4.155 |
| R medio antes de costos | −0,021 | −0,013 |
| R neto medio (IC 95 %) | −0,169 (± 0,076) | −0,151 (± 0,039) |
| R neto con costos × 2 | −0,317 | −0,288 |
| Aciertos (IC 95 %) | 33,2 % [30,5; 35,9] | 33,6 % [32,2; 35,1] |
| Con el +1 de "FVG u order block" en el puntaje | 1.089 (95 %) | 2.641 (64 %) |
| Salidas por stop | 701 (61 %) | 2.411 (58 %) |
| R neto alcistas / bajistas | −0,117 / −0,245 | −0,142 / −0,162 |

- **F − R por tercil de tamaño del FVG:** chico +0,034; medio −0,040; grande −0,049.
- **Variante "toque en `t` o en `t − 1`":** 1.834 operaciones, F′ − R′ = +0,012 R (descriptivo: no cambia la conclusión).
- Antes de costos, tanto F como R están cerca de cero: lo que hace la pérdida son los costos, como en R001–R003.

### Estrategia filtrada: solo las operaciones F (descriptivo, no vota, punto 14)

Se repitió el recorrido de cada ventana permitiendo entrar solo en las velas de toque (4 pasadas hasta que todas las operaciones fueron F). Los 5 criterios del programa 4 §7 contra comprar y mantener el CFD, con N = 361:

| | US100 EUR | US100 USD | US500 EUR | US500 USD |
|---|---|---|---|---|
| Operaciones | 1.014 | 1.014 | 971 | 970 |
| R neto medio | −0,039 | −0,040 | −0,219 | −0,220 |
| Retorno medio por mes | −0,33 % | −0,35 % | −1,51 % | −1,52 % |
| Comprar y mantener por mes | +1,46 % | +1,36 % | +0,99 % | +0,90 % |
| Alfa por mes (p) | −0,18 % (0,71) | −0,19 % (0,74) | −1,47 % (1,00) | −1,48 % (1,00) |
| Sharpe deflactado | 0,0002 | 0,0002 | 0,0000 | 0,0000 |
| Criterios cumplidos | 0 de 5 | 0 de 5 | 0 de 5 | 0 de 5 |

La estrategia filtrada pierde dinero y queda muy por debajo de comprar y mantener en los dos índices.

## 4. Error de código en la primera corrida de H5.1

- **Qué fue:** el ejemplo calculaba las ventanas del walk-forward desde la fecha pedida (01-01-2011 00:00) en vez de desde la **primera vela cargada** (como `research_round::load` y H5.2). Con eso la validación empezaba el 01-01-2013 (154 meses) y no el 01-02-2013 (153 meses) que fija el pre-registro. Es un error de código, no de la regla.
- **Cuándo se vio:** al leer la primera línea de la salida, después de que la corrida terminó (con sus resultados ya impresos).
- **Corrección:** commit `3f03e3c`; la segunda corrida da exactamente los conteos del pre-registro.
- **Resultado anterior (corrida 1, no vale):** 45.551 y 41.187 eventos con placebo; media de D̄ −0,0142 ATR, t = −1,12, p = 0,867; criterio 2: −0,016 y −0,013; mitades −0,004 y −0,032; con `r_4` +0,005 (ese criterio sí se cumplía); veredicto **no pasa**. Test de suerte: percentil 0,1.
- **Resultado nuevo (corrida 2, la que vale):** el de §2, **no pasa**. Como el mes de más cambia todos los sorteos que vienen después en el mismo flujo, las cifras se mueven un poco; la conclusión no cambia.
- En el registro: R034 (corrida 1) y R035 (corrida 2, con `compara_con` R034).

## 5. Lecturas y supuestos declarados

El pre-registro congelado (§13.3) fijó la implementación antes de correr. Lo que quedó por interpretar, siempre con la lectura conservadora o la literal:

1. **Spread de la estrategia (H5.2).** El recorrido del backtest (el mismo del `diagnostico` de #456 que nombra §5.1) usa **un solo spread**, el de `config/instruments.toml` (US100 1,6; US500 0,9), a cualquier hora; no usa el spread más alto fuera de la sesión de `docs/research/ig-spreads-2026-11.md` que menciona §1. Se dejó así porque §5.1 pide las operaciones del `diagnostico` tal cual; con el spread más alto, las dos clases (F y R) pagarían más, y H5.2 compara una contra la otra.
2. **Noticias.** No hay interpretaciones de noticias guardadas para 2013–2025 (el corte del modelo es 12-2025, ADR 0008) ni calendario histórico de alto impacto: la estrategia corre sin el ajuste de noticias y sin el filtro de noticias, igual que en el `diagnostico` de #456.
3. **Eventos sin placebo.** Un 1 % de los eventos (480 y 436) no tiene ninguna vela candidata y sale del análisis de los dos lados, como manda §4.1.
4. **H5.1 sin escenarios de financiación.** H5.1 no opera: el resultado es el mismo con EUR y USD.
5. **Costos descriptivos de H5.1.** El spread fuera de la sesión es el de la franja más frecuente de `docs/research/ig-spreads-2026-11.md` (US100 2,5; US500 1,6), fijado en `[program5]` al congelar.

## 6. Qué sigue

- **H5.1 y H5.2 se cierran.** Por §10 del pre-registro, no se prueban variantes sobre estos datos (otro tamaño mínimo, otra espera, otro horizonte, 1 hora como principal, un solo índice, etc.: todo eso contaría como pruebas nuevas, con su costo en el presupuesto).
- El programa 5 no tiene hipótesis vivas. El fuera de muestra sigue sin usar.
- Nada cambia en la estrategia en uso, en `config/liquidity.toml` ni en el paper.

## 7. Registro

Filas nuevas en `registro-de-resultados.csv`: **R034** (H5.1, corrida 1 con el error de código), **R035** (H5.1, la que vale), **R036** (H5.1 en 1 hora, descriptivo), **R037** (H5.2) y **R038** (estrategia filtrada, descriptivo).
