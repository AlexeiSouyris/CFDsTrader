# Programa Nasdaq, H2: días de CPI y empleo en el CFD US100 (resultados)

Issue #526 · 25-11-2026 · regla congelada en `2026-11-programa-nasdaq-preregistro.md` (#528, SHA-256 `1979edb79421195e4792a30427e871aa24148a3ba5be4d9496793b2c5fca8afe`, verificado antes de correr: coincide con el `.sha256`). Calendario del BLS cargado en #610 (ADR 0016). Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real.

## Resumen en simple

- **Qué se probó:** los días en que EE. UU. publica el dato de inflación (CPI) o el de empleo, comprar el US100 a las 08:00 (hora de Nueva York), media hora antes del dato, y venderlo al final de la sesión de ese mismo día. 298 días entre 02-2013 y 10-2025, con todos los costos del CFD (la entrada paga el spread más ancho de fuera de la sesión).
- **Veredicto: H2 NO PASA** (en los dos escenarios de financiación). Falla los cinco criterios. Se cierra y no se prueban variantes.
- **Lo que se vio:** antes de costos, el US100 subió en promedio **+10,7 pb** (0,11 %) esos días; después de costos, **+6,8 pb**. Pero:
  - un día normal cualquiera también sube algo (el Nasdaq subió mucho en estos años), así que contra **comprar y mantener** la ventaja queda **negativa**: α = −0,05 % por mes (p = 0,64; hacía falta p < 0,0125);
  - el Sharpe deflactado da **0,0004** (hace falta 0,95);
  - lo poco positivo está en **2013–2020** (+13 pb neto por día); en **2021–2025** da **−3,7 pb**;
  - una cartera mitad comprar y mantener y mitad H2 rinde **peor** por unidad de riesgo que comprar y mantener solo.
- **El costo no es el problema:** ≈ 3,9 pb por día. Simplemente, en esta muestra, estar comprado solo los días de dato no rinde más que estar comprado siempre.
- **Con el presupuesto aprobado (1,25 %) H2 queda cerrada.** Con H1 y H2 cerradas, el programa Nasdaq (familia E del programa 2) termina sin ninguna hipótesis que pase. El fuera de muestra sigue sin tocar.

## 1. Cómo se corrió

- Ejemplo `programa_nasdaq_h2` de `cfds-backtest` (`engine/crates/backtest/examples/programa_nasdaq_h2.rs`), con la regla pura `research::macro_day` (`macro_day_windows`) y los retornos y costos de `managed_returns_with_spreads`: la misma función que H1, la familia V y comprar y mantener (`managed_returns`), con un único agregado: la ejecución de la entrada de las 08:00 ET usa el spread de fuera de sesión (2,5 puntos, `[macro_day]` de `config/research.toml`) y su slippage (0,1 × 2,5 + 1 tick). La salida usa el spread de sesión (1,6 puntos). Tests con datos sintéticos en `engine/crates/backtest/tests/research_nasdaq_h2.rs` (horario de verano e invierno, cierre temprano, Viernes Santo, otra hora, mismo día, publicación, reprogramación y cancelación, velas faltantes, no repintado y el costo exacto de la entrada).
- Base real en **solo lectura** (`default_transaction_read_only=on`): velas de futuros NQ (y ES, YM para el descriptivo) hasta el 01-11-2025 (el fuera de muestra **no se cargó**) y `official_releases` (`source = 'bls'`, series `cpi` y `employment_situation`) publicado antes de esa fecha.
- Una corrida continua sobre 01-02-2013 → 01-11-2025 (153 meses, §2.6 del pre-registro). Comprar y mantener: una cuenta nueva por cada una de las 51 ventanas, igual que H1 y R018 (+1,456 % por mes, Sharpe mensual 0,293 en EUR; +1,357 % y 0,273 en USD).

### Registro de corridas

| # | Hora (UTC) | Comando (desde `engine/`, código del commit `f57adaa` de la rama, antes del squash) | Nota |
|---|---|---|---|
| 1 | 25-11-2026 11:45:21 | `programa_nasdaq_h2 US100 US500 US30 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida … --eventos …` | primera y única corrida EUR |
| 2 | 25-11-2026 11:47:05 | `programa_nasdaq_h2 US100 US500 US30 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida … --eventos …` | primera y única corrida USD |

No hubo corridas repetidas ni errores de código encontrados después de ver los resultados.

### Lista de eventos (antes de calcular retornos)

- **305 datos** del BLS con fecha dentro del período (CPI y empleo). **298 operados.** Ningún día tuvo CPI y empleo a la vez, así que no hubo que juntar ninguno.
- **7 no operados**, por regla:
  - 5 sin sesión de contado (Viernes Santo con dato de empleo o de CPI): empleo del 03-04-2015, CPI del 14-04-2017, CPI del 10-04-2020, empleo del 02-04-2021 y empleo del 07-04-2023;
  - 2 por falta de la vela de la salida en los datos: CPI de 02-2013 y CPI de 08-2016.
- Detalle dato por dato (fecha, serie, estado, entrada, salida, retorno bruto, neto y × 1,5): `2026-11-programa-nasdaq-h2-eventos-eur.csv` y `…-usd.csv` (idénticos en US100: ninguna posición cruza la noche). Retornos mensuales: `2026-11-programa-nasdaq-h2-retornos-eur.csv` y `…-usd.csv`.

## 2. Criterios (US100, el que vota)

| Criterio (pre-registro §3.4 y §4.3) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Alfa contra comprar y mantener, p < 0,0125 | α = −0,052 % por mes (≈ −0,63 % por año), β 0,126, t −0,36, **p 0,641** | α = −0,039 %, t −0,27, **p 0,607** | **no** |
| 2. Sharpe deflactado de la ventaja ≥ 0,95 (N = 356) | Sharpe mensual −0,033, SR0 0,239, **DSR 0,0004** | Sharpe −0,025, **DSR 0,0005** | **no** |
| 3. Ventaja media > 0 en las dos mitades | 2013–2020: +0,032 %; 2021–2025: **−0,191 %** | +0,044 % / **−0,174 %** | **no** |
| 4. Alfa con costos × 1,5 > 0 | **−0,080 %** (p 0,71) | **−0,061 %** (p 0,67) | **no** |
| 5. Mezcla 50/50 con Sharpe mayor que comprar y mantener | 0,273 < 0,293 | 0,256 < 0,273 | **no** |

**H2 no pasa**: falla los cinco criterios en los dos escenarios (alcanzaba con fallar uno).

Referencias sin voto: DSR con N = 100: 0,0015 (EUR); con N = 1000: 0,0001; con el N vigente al correr (359, el del programa 4): 0,0004. Costos × 2: α = −0,108 % por mes (EUR).

H2 en sí: +0,132 % por mes (los dos escenarios: sin noches no hay financiación), Sharpe mensual 0,077; con posición en los 153 meses (dos días por mes, unas 7 horas y 45 minutos cada uno).

## 3. Descriptivo (sin voto)

### Por día de dato, US100 (EUR y USD dan igual)

| Corte | Días | Bruto (media, t) | Neto (media, t) |
|---|---|---|---|
| Todos | 298 | +10,73 pb (t 1,54) | +6,84 pb (t 0,98) |
| 2013–2020 | 185 | +18,50 pb (t 2,73) | +13,27 pb (t 1,96) |
| 2021–10/2025 | 113 | −1,98 pb (t −0,14) | −3,69 pb (t −0,25) |
| Solo CPI | 148 | +15,40 pb (t 1,46) | +11,54 pb (t 1,09) |
| Solo empleo | 150 | +6,13 pb (t 0,67) | +2,20 pb (t 0,24) |
| CPI y empleo el mismo día | 0 | — | — |

Costo medio por día: **3,90 pb** (× 1,5: 5,84 pb). Desvío por día: 120 pb (en H1 fue 86 pb por evento).

Cómo leerlo: el t de 2,73 de 2013–2020 parece alto, pero (a) la prueba que vota es el alfa **mensual contra comprar y mantener**, y en esos años el índice también subía los días sin dato; (b) estos cortes son descriptivos y se miraron varios; (c) en 2021–2025 desaparece. Ningún corte de esta tabla se puede usar ahora como regla nueva (pre-registro §6).

### La misma regla en US500 y US30 (contexto, sin voto)

Spread de la entrada fuera de sesión: US500 1,6 puntos y US30 5,0 puntos (franja 08–13 UTC de `docs/research/ig-spreads-2026-11.md`, en `[macro_day]`).

| Índice | Bruto por día (t) | Neto por día (t) | α mensual (p), EUR | DSR | Mitades |
|---|---|---|---|---|---|
| US100 | +10,73 pb (1,54) | +6,84 pb (0,98) | −0,052 % (0,64) | 0,0004 | + / − |
| US500 | +8,21 pb (1,50) | +2,83 pb (0,52) | −0,059 % (0,70) | 0,0002 | + / − |
| US30 | +5,81 pb (1,16) | +3,56 pb (0,71) | −0,019 % (0,58) | 0,0008 | + / − |

El patrón es el mismo en los tres índices: algo positivo antes de 2021, nada después y, contra comprar y mantener, ninguna ventaja.

### Comparación con lo publicado

Savor y Wilson (2013) encuentran que el mercado de acciones de EE. UU. rinde más los días de datos macro programados (inflación, empleo, FOMC) que los demás días, con datos de 1958 a 2009 (resumen del trabajo, no releído en esta tarea). Acá, con 2013–2025 y una ventana dentro del día (08:00 ET a la última vela de la sesión), el efecto antes de costos es chico (+11 pb en US100) y no supera lo que el índice sube un día cualquiera de esta muestra. Con el desvío observado (120 pb por día), la propuesta ya avisaba un poder bajo (≈ 26 %, pre-registro §4.4).

## 4. Qué sigue

- **H2 se cierra.** No se prueban variantes (otra hora, solo CPI, solo hasta 2020, otro índice): todas serían pruebas nuevas sin presupuesto (programa 2 en 0 %).
- Con H1 y H2 cerradas, **el programa Nasdaq no tiene hipótesis vivas**. H3 y H4 de la propuesta siguen en reserva sin presupuesto.
- El fuera de muestra (desde el 01-11-2025) sigue sin tocar.
- Registro de resultados: fila **R031** (H2 en US100, vota) y **R032** (la misma regla en US500 y US30, descriptivo).

## Ambigüedades y cómo se resolvieron (lectura conservadora)

- **Días de cierre del gobierno (2013 y 2025).** El calendario cargado en #610 sigue la regla del ADR 0016: si el informe anterior anunció una fecha antes del cierre, esa fila queda como `scheduled`, aunque ese día el dato no saliera, porque no hay una fecha documentada de cuándo se supo que se postergaba. Por eso H2 operó, como "día de dato", tres días en que el BLS no publicó nada: empleo del 04-10-2013 (+61 pb neto), CPI del 16-10-2013 (+63 pb) y empleo del 03-10-2025 (−51 pb). Se aplicó la regla congelada (§2.3: vale la última fila publicada antes de la decisión) tal cual, sin sacarlos a mano (§6 lo prohíbe). Juntos suman +0,73 % en 298 días (unos +0,25 pb por día en promedio): no cambian el veredicto, que falla los cinco criterios por mucho. Las fechas nuevas de esos datos (22-10-2013 y 30-10-2013; 24-10-2025 y 20-11-2025) **no están** en la tabla porque no tienen una fecha documentada de cuándo se anunciaron, así que no se operaron.
- **Spread de la entrada en US500 y US30.** El pre-registro fija 2,5 puntos fuera de sesión solo para el US100. Para el descriptivo de los otros dos índices se usó la mediana de IG de la misma franja horaria (US500 1,6; US30 5,0), escrita en `config/research.toml` antes de correr. No vota.
- **N del Sharpe deflactado.** Vota N = 356 (el del congelado). El N vigente al correr (359) se informa aparte, como pide §4.3; da lo mismo (0,0004).

## Límites

- Los costos son los del catálogo, con la tasa de financiación del US100 pendiente con IG (por eso los dos escenarios; acá no importa, porque ninguna posición cruza la noche). El spread fuera de sesión (2,5 puntos) es el de noviembre de 2026; no hay spread medido del CFD para esas fechas.
- Las velas son de futuros NQ, no del CFD.
- Las horas son las del calendario del BLS (08:30 ET); la entrada es media hora antes, así que un error de ± 15 minutos en la hora no cambia la regla.
- 298 días: muestra moderada, con mucho ruido por día (120 pb de desvío).
