# Programa Nasdaq, H1: deriva antes del comunicado del FOMC en el CFD US100 (resultados)

Issue #526 · 24-11-2026 · regla congelada en `2026-11-programa-nasdaq-preregistro.md` (#528, SHA-256 `1979edb79421195e4792a30427e871aa24148a3ba5be4d9496793b2c5fca8afe`, mergeado antes de correr). Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real.

## Resumen en simple

- **Qué se probó:** comprar el US100 a las 14:00 (hora de Nueva York) del día hábil anterior a cada reunión programada de la Reserva Federal y venderlo 30 minutos antes del comunicado. 101 reuniones entre 02-2013 y 10-2025, con todos los costos del CFD.
- **Veredicto: H1 NO PASA** (en los dos escenarios de financiación). Se cierra y no se prueban variantes.
- **Lo que se vio:** antes de costos, el US100 subió en promedio **+20 pb** (0,20 %) en esas ~23 horas; después de costos, **+16 pb**. Parece algo, pero:
  - es **muy poco comparado con lo que se mueve el índice** esos días (desvío de 86 pb por evento), así que no se distingue del azar con la vara fijada (p = 0,12 contra 0,0125);
  - **en 2013–2020 casi no existió** (+4 pb neto por evento; la ventaja mensual fue levemente negativa). Todo lo positivo está en **2021–2025** (+36 pb neto), justo al revés de lo que pide el criterio de las dos mitades;
  - el Sharpe deflactado, que descuenta las 356 pruebas que ya hizo el proyecto, da **0,035** (hace falta 0,95).
- **Lo que sí se confirma:** el costo **no** es el problema (≈ 3,6 a 3,9 pb por evento, como estimó la propuesta). Si hubiera un efecto grande y estable, los costos no lo taparían. Simplemente, en esta muestra no es lo bastante grande ni estable.
- **Con el presupuesto aprobado (1,25 %) H1 queda cerrada.** H2 (CPI y empleo) sigue congelada, esperando el calendario del BLS.

## 1. Cómo se corrió

- Ejemplo `programa_nasdaq_h1` de `cfds-backtest` (`engine/crates/backtest/examples/programa_nasdaq_h1.rs`), con la regla pura `research::pre_fomc` y los retornos y costos de `managed_returns` (la misma función de la familia V y de comprar y mantener, ronda 3). Tests con datos sintéticos en `engine/crates/backtest/tests/research_nasdaq.rs`.
- Base real en **solo lectura** (`default_transaction_read_only=on`): velas de futuros NQ (y ES, YM para el descriptivo) hasta el 01-11-2025 (el fuera de muestra **no se cargó**) y `official_releases` (FOMC) publicado antes de esa fecha.
- Una corrida continua sobre 01-02-2013 → 01-11-2025 (153 meses, §2.6 del pre-registro). Comprar y mantener: una cuenta nueva por cada una de las 51 ventanas (igual que R018). El comprar y mantener del US100 dio **+1,456 % por mes, Sharpe mensual 0,293** (EUR), igual que R018 y los chequeos de #512.

### Registro de corridas

| # | Hora (UTC) | Comando (desde `engine/`, código del commit `7c27f8d` de la rama, antes del squash) | Nota |
|---|---|---|---|
| 1 | 24-11-2026 20:22:58 | `programa_nasdaq_h1 US100 US500 US30 --escenario-financiacion EUR=../docs/analysis/2026-11-eur-eonia-estr.csv --salida … --eventos …` | primera y única corrida EUR |
| 2 | 24-11-2026 20:23:16 | `programa_nasdaq_h1 US100 US500 US30 --escenario-financiacion USD=../docs/analysis/2026-11-usd-effr-sofr.csv --salida … --eventos …` | primera y única corrida USD |

No hubo corridas repetidas ni errores de código encontrados después de ver los resultados.

### Lista de eventos (antes de calcular retornos)

- **101 reuniones programadas operadas** (las 101 esperadas en el pre-registro). Ninguna se perdió por velas faltantes, cierres tempranos o superposición.
- **4 comunicados no programados no operados**, por regla (§2.4): 11-10-2019, 03-03-2020, 15-03-2020 y 23-03-2020.
- Detalle evento por evento (hora del comunicado, regla de hora, conferencia, entrada, salida, retorno bruto, neto y × 1,5): `2026-11-programa-nasdaq-h1-eventos-eur.csv` y `…-usd.csv`. Retornos mensuales: `2026-11-programa-nasdaq-h1-retornos-eur.csv` y `…-usd.csv`.

## 2. Criterios (US100, el que vota)

| Criterio (pre-registro §3.4) | EUR | USD | ¿Pasa? |
|---|---|---|---|
| 1. Alfa contra comprar y mantener, p < 0,0125 | α = +0,070 % por mes (≈ +0,84 % por año), β 0,026, t 1,15, **p 0,125** | α = +0,071 %, t 1,18, **p 0,120** | **no** |
| 2. Sharpe deflactado de la ventaja ≥ 0,95 (N = 356) | Sharpe mensual 0,101, SR0 0,239, **DSR 0,035** | Sharpe 0,102, **DSR 0,036** | **no** |
| 3. Ventaja media > 0 en las dos mitades | 2013–2020: **−0,018 %**; 2021–2025: +0,214 % | −0,017 % / +0,215 % | **no** |
| 4. Alfa con costos × 1,5 > 0 | +0,060 % (p 0,16) | +0,061 % (p 0,15) | sí |
| 5. Mezcla 50/50 con Sharpe mayor que comprar y mantener | 0,304 > 0,293 | 0,284 > 0,273 | sí |

**H1 no pasa**: falla los criterios 1, 2 y 3 en los dos escenarios (alcanzaba con fallar uno).

Referencias sin voto: DSR con N = 100: 0,086 (EUR); con N = 1000: 0,016. Costos × 2: α = +0,050 % por mes (EUR).

H1 en sí: +0,108 % por mes (EUR), Sharpe mensual 0,154; con posición en 109 de los 153 meses (unas 23 horas por reunión).

## 3. Descriptivo (sin voto)

### Por evento, US100 (EUR; USD casi igual)

| Corte | Eventos | Bruto (media, t) | Neto (media, t) |
|---|---|---|---|
| Todos | 101 | +19,98 pb (t 2,33) | +16,41 pb (t 1,91) |
| 2013–2020 | 62 | +9,06 pb (t 1,04) | +4,36 pb (t 0,50) |
| 2021–10/2025 | 39 | +37,35 pb (t 2,17) | +35,57 pb (t 2,07) |
| Con conferencia de prensa | 78 | +27,05 pb (t 2,60) | +23,99 pb (t 2,30) |
| Sin conferencia | 23 | −3,99 pb (t −0,33) | −9,31 pb (t −0,76) |

Costo medio por evento: **3,57 pb** EUR / 3,90 pb USD (× 1,5: 5,36 / 5,86 pb). Desvío por evento: 86 pb (la propuesta supuso ≈ 130 pb).

Cómo leerlo: el t de 2,33 antes de costos parece alto, pero (a) la prueba que vota es el alfa **mensual** con su nivel 0,0125 y no un t por evento, (b) estos cortes son descriptivos y se miraron varios, y (c) el efecto se concentra en un subperíodo. Los cortes "con / sin conferencia" y "desde 2021" **no** se pueden usar ahora como regla nueva (pre-registro §6).

### La misma regla en US500 y US30 (contexto, sin voto)

| Índice | Bruto por evento (t) | Neto por evento (t) | α mensual (p), EUR | DSR | Mitades |
|---|---|---|---|---|---|
| US100 | +19,98 pb (2,33) | +16,41 pb (1,91) | +0,070 % (0,125) | 0,035 | − / + |
| US500 | +14,47 pb (2,42) | +10,11 pb (1,68) | +0,051 % (0,105) | 0,041 | − / + |
| US30 | +11,72 pb (2,29) | +9,31 pb (1,81) | +0,051 % (0,076) | 0,064 | + / + |

El patrón es el mismo en los tres índices y algo más grande en el Nasdaq en puntos básicos (es también el más volátil). Ninguno se acerca a pasar.

### Comparación con lo publicado

Lucca y Moench (2015) informan ≈ 49 pb en las 24 horas antes del anuncio en el S&P 500 entre 1994 y 2011 (cifra de su resumen, no releída). Acá, con 2013–2025 y una ventana de ≈ 23 horas, el S&P 500 (US500) da +14 pb antes de costos y el US100 +20 pb: **un tercio a la mitad** de lo publicado, y concentrado en 2021–2025. Coincide con la idea (no verificada en esta tarea) de que el efecto se achicó después de su publicación. Con el desvío observado (86 pb), el poder para detectar +20 pb por evento con la vara mensual era bajo, como avisó la propuesta (§2: ≈ 44 % si el efecto era la mitad del publicado).

## 4. Qué sigue

- **H1 se cierra.** No se prueban variantes (otra hora, solo reuniones con conferencia, solo desde 2021, otro índice): todas serían pruebas nuevas sin presupuesto (programa 2 en 0 %).
- **H2** (días de CPI y empleo) sigue congelada con su 1,25 %, a la espera de la carga del calendario del BLS (#505). Su poder esperado es bajo (≈ 26 %).
- El fuera de muestra (desde el 01-11-2025) sigue sin tocar.
- Registro de resultados: fila **R027** (H1 en US100, vota) y **R028** (la misma regla en US500 y US30, descriptivo).

## Límites

- Los costos son los del catálogo, con la tasa de financiación del US100 pendiente con IG (por eso los dos escenarios). El spread de la entrada y la salida es el de sesión (1,6 puntos): las dos caen dentro de la sesión de contado.
- Las velas son de futuros NQ, no del CFD; la diferencia de financiación se corrige con la base del futuro (#470).
- Las horas de los comunicados de 2013 a 2015 vienen de la regla de `config/official_calendar.toml` (14:00 ET desde el 13-03-2013), no de la página de cada comunicado; el margen de 30 minutos de la salida cubre ± 15 minutos.
- Muestra chica: 101 eventos.
