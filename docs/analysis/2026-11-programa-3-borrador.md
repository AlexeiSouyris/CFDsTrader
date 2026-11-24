# Programa 3: pre-registro (borrador). Tendencia multi-activo en CFD de futuros de IG

> **Borrador del dueño (24-11-2026), sin congelar.**
>
> Traducción fiel del borrador "CFDsTrader — Program 3 Pre-registration: Multi-asset trend on IG futures CFDs" (24-11-2026, @Alexei Souyris), Issue #513. No se cambió ninguna regla. Lo que choca con `CLAUDE.md`, la spec o el código está en las **Observaciones del agente** (§A), los datos que el agente pudo averiguar están en §B y §C, y lo que tiene que decidir el dueño está en §D.
>
> Mientras esté en borrador **no se congela ni se corre nada** con estas reglas.

## Estado e hipótesis

**Estado:** borrador, sin congelar. Se congela solo después de cerrar todos los puntos de "Puntos abiertos antes de congelar". Congelar significa commitear este archivo con fecha y hora y su hash SHA-256, antes de que corra ningún código con estas reglas.

**Hipótesis:** una señal de tendencia fija, aplicada a un conjunto diversificado de CFD de futuros de IG, agrega retorno ajustado por riesgo a comprar y mantener los índices de EE. UU., después de costos realistas.

**Prueba principal:** el intercepto (alfa) de la regresión de los retornos mensuales de la cartera de tendencia sobre los retornos mensuales de comprar y mantener es mayor que cero.

**Por qué este programa:** la tendencia sobre un solo índice (ronda 2) llegó a un Sharpe de alrededor de 0,30. La evidencia publicada sobre tendencia se apoya en la amplitud entre clases de activos, y los CFD de futuros de IG no tienen cargo de financiación overnight.

## Universo

El universo tiene 16 mercados candidatos en 4 clases de activos. Se fija antes de cualquier resultado. Un mercado se descarta solo si IG no ofrece un CFD de futuros sobre él o si Databento no tiene su historia. Esa revisión se hace antes de congelar, nunca después de un resultado.

| Clase de activo | Mercado | Futuro usado para la historia | Historia desde |
|---|---|---|---|
| Renta variable | S&P 500 | ES | 2011 |
| Renta variable | Nasdaq 100 | NQ | 2011 |
| Renta variable | Russell 2000 | RTY | 2017 |
| Renta variable | Nikkei 225 | NKD | a confirmar |
| Renta variable | Euro Stoxx 50 | FESX | a confirmar |
| Tasas | Nota de EE. UU. a 10 años | ZN | a confirmar |
| Tasas | Bono de EE. UU. a 30 años | ZB | a confirmar |
| Tasas | Bund alemán | FGBL | a confirmar |
| Divisas | EUR/USD | 6E | a confirmar |
| Divisas | USD/JPY | 6J | a confirmar |
| Divisas | GBP/USD | 6B | a confirmar |
| Divisas | AUD/USD | 6A | a confirmar |
| Materias primas | Oro | GC | a confirmar |
| Materias primas | Plata | SI | a confirmar |
| Materias primas | Crudo WTI | CL | a confirmar |
| Materias primas | Cobre | HG | a confirmar |

El Dow (YM) queda excluido por redundante con ES. Un mercado entra en la cartera cuando tiene al menos 12 meses de historia en una fecha de rebalanceo, lo que cubre el inicio tardío de RTY.

## Datos y división de la muestra

- **Precios:** velas diarias armadas con los datos de futuros de 1 minuto de Databento, a la hora de liquidación (*settlement*) de la bolsa de cada mercado. Los contratos se ajustan hacia atrás (*back-adjusted*) y se renuevan en las mismas fechas de renovación que usa IG para sus CFD de futuros.
- **Holdout:** todo lo que va desde el 01-11-2025 queda sin abrir. Se usa solo si se cumplen todos los criterios para pasar.
- **Walk-forward:** las mismas 51 ventanas de tres meses de los programas anteriores (febrero de 2013 a octubre de 2025, 153 meses). Los datos de 2011 a 2012 se usan solo para el calentamiento de las ventanas de retorno (*lookbacks*).
- **Sin parámetros ajustados:** todas las reglas de abajo se fijan de antemano. El walk-forward solo garantiza que las estimaciones de volatilidad y las señales usen datos disponibles en ese momento.

## Reglas de la señal

La señal de cada mercado es una mezcla en partes iguales de tres ventanas de tendencia, recalculada una vez por mes. Se permiten posiciones largas y cortas.

1. El último día hábil de cada mes, al cierre, se calcula el retorno total de cada mercado a 1, 3 y 12 meses (21, 63 y 252 días hábiles).
2. Señal = el promedio de los signos de esos tres retornos. Valores posibles: −1, −1/3, +1/3, +1.
3. Las órdenes salen en la apertura de la sesión siguiente. No se usa ninguna información posterior al cierre de la señal.
4. Banda sin operar: una posición se cambia solo si el nuevo tamaño objetivo difiere del tamaño actual en más del 25 % del objetivo.

Las ventanas siguen la literatura estándar de seguimiento de tendencia. No se ajustan con estos datos, y en este programa no se va a probar ninguna otra ventana.

## Tamaño y riesgo

Cada mercado recibe el mismo presupuesto de riesgo fijo, para que ningún mercado domine la cartera.

```text
Nocional_i = Señal_i × Capital × 2,5 % / σ_i
```

σ_i es la volatilidad anualizada del mercado calculada con los retornos diarios de los 60 días hábiles anteriores, tal como se conoce al cierre de la señal.

- **Tope de apalancamiento:** el nocional bruto total tiene un tope de 4 × el capital. Si el margen de IG de todas las posiciones superara el 50 % del capital, todas las posiciones se reducen por el mismo factor.
- **Stop:** cada posición tiene un stop fijo puesto donde su pérdida sería el 1 % del capital, lo que mantiene la regla del 1 % del proyecto. Con este tamaño son unas 6 desviaciones estándar diarias, así que funciona como stop de desastre. Un mercado que salta por el stop queda sin posición hasta el próximo rebalanceo mensual. Los gaps se ejecutan al siguiente precio disponible, no al precio del stop.
- El corte por pérdida diaria del 3 % y el freno de cuenta del 15 % siguen sin cambios.
- **Cuentas:** la corrida principal usa 100.000 €. Una segunda corrida con 10.000 € informa cuántas posiciones objetivo quedan por debajo del tamaño mínimo de IG.

## Modelo de costos

Los costos nunca son cero. Las renovaciones cuentan como operaciones, porque los CFD de futuros de IG vencen y hay que volver a abrirlos.

| Costo | Regla |
|---|---|
| Spread | El spread publicado por IG para el CFD de futuros de cada mercado, recogido y congelado antes de la corrida, pagado en cada entrada, salida y renovación |
| Slippage | Otra vez la mitad del spread en cada ejecución |
| Financiación overnight | Ninguna, porque IG la incluye en el precio del CFD de futuros; el *carry* ya está en los precios de futuros usados |
| Conversión de divisa | La comisión de IG por convertir a EUR las ganancias y pérdidas que no están en EUR (tasa a confirmar) |
| Prueba de estrés | Todos los costos × 1,5 (criterio para pasar) y × 2 (solo se informa) |

## Benchmark

El benchmark es comprar y mantener en partes iguales ES, NQ y YM, mantenidos a través de los mismos CFD de futuros de IG y con el mismo modelo de costos, incluidos los costos de renovación. Esta es la comparación equivalente.

Comprar y mantener sin costos (alrededor de 1,10 % por mes, Sharpe alrededor de 0,90) también se informa, solo como contexto.

## Criterios para pasar

La idea pasa solo si cumple los seis criterios. Los criterios 1 y 5 reemplazan la vieja prueba de "le gana a comprar y mantener por sí sola", porque puede valer la pena agregar un diversificador aunque su propio Sharpe sea menor.

1. **Alfa:** regresión del retorno mensual de la cartera de tendencia sobre el retorno mensual del benchmark (MCO, errores de Newey-West con 3 rezagos). Prueba unilateral de intercepto > 0 con p < 0,0125.
2. **Sharpe deflactado ≥ 0,95**, con la cantidad de pruebas del registro al momento de congelar (al menos 355). Se aplica a la mezcla 50/50 del criterio 5 (ver Puntos abiertos).
3. **Consistencia:** alfa > 0 en las dos mitades (2013–2019 y 2020–2025), y contribución positiva de al menos 3 de las 4 clases de activos.
4. **Costos:** la estimación del alfa sigue por encima de 0 con todos los costos × 1,5.
5. **Valor en cartera:** una mezcla de 50 % benchmark + 50 % tendencia, escalada a la volatilidad del benchmark, tiene un Sharpe mayor que el benchmark solo.
6. **Factibilidad:** con 10.000 €, al menos el 80 % de las posiciones objetivo se pueden abrir con los tamaños mínimos de IG. Si no, el resultado cuenta solo como investigación y no puede pasar a real con ese tamaño.

## Presupuesto de error

- El programa 3 recibe un presupuesto nuevo de falsos positivos del 5 %. Esta prueba usa 1,25 %, lo que deja 3,75 %.
- El registro suma una prueba por esta prueba.
- El 3,75 % restante solo se puede gastar con un pre-registro nuevo y fechado. Candidatos probables: una señal de *carry* sobre el mismo universo, o la deriva overnight después de cierres con caídas fuertes en MES.
- Ninguna variante de esta prueba (otras ventanas, otros objetivos de volatilidad, otros universos) puede usar el presupuesto.

## Lo que no se permite después de ver resultados

- Cambiar las ventanas, el día de rebalanceo, la banda, el objetivo de volatilidad, la distancia del stop, el tope de apalancamiento o los supuestos de costos.
- Descartar mercados o clases de activos que pierden plata.
- Informar solo el escenario de costos o el subperíodo más favorable.
- Abrir el holdout antes de que pasen los seis criterios.

Se permiten correcciones de errores (*bugs*). Cada corrección se registra con su fecha, se vuelve a correr todo y el resultado de antes de la corrección queda en el registro.

## Puntos abiertos antes de congelar

- Confirmar cuáles de los 16 mercados ofrece IG como CFD de futuros, con spread, tamaño de contrato, operación mínima y margen minorista de cada uno.
- Confirmar la historia en Databento desde 2011 de los contratos de CME, Eurex e ICE marcados "a confirmar".
- Conseguir el calendario de renovaciones de IG para los CFD de futuros, para que las renovaciones del backtest coincidan con las reales.
- Confirmar la comisión de IG por conversión de divisa sobre las ganancias y pérdidas que no están en EUR.
- Decidir si el criterio 2 (Sharpe deflactado) se aplica a la mezcla 50/50 o a la tendencia sola.
- Confirmar que el stop de desastre del 1 % se acepta como cumplimiento de la regla del stop obligatorio del proyecto.
- Congelar: commitear este archivo, registrar su hash SHA-256 y su fecha y hora, y agregar la prueba al registro.

---

# Anexos del agente (no son parte del borrador)

Lo que sigue lo agregó el agente (Claude Code) el 24-11-2026 para el Issue #513. No cambia ninguna regla del borrador.

## A. Observaciones del agente

Lo que choca con `CLAUDE.md`, la spec (`docs/spec/fase-1-especificacion.md`), la configuración o el código. **No se cambió nada del borrador**: cada punto es para que el dueño lo resuelva antes de congelar.

**Choques con la spec y la configuración de riesgo**

1. **Stop.** La spec (§8) pone el stop por estructura (mínimo de la estructura − 0,5 × ATR), con TP1 en 2R, trailing a 2 ATR y salida por tiempo a las 32 velas, y el filtro 7 (§7) no abre si el stop está a menos de 0,5 ATR o a más de 3 ATR. El borrador usa un stop fijo a una pérdida del 1 % del capital (≈ 6 desviaciones diarias), sin TP ni trailing: es otro tipo de stop. `CLAUDE.md` §5 exige stop obligatorio; el borrador lo cumple con el stop de desastre, pero **que eso cuente como "el" stop obligatorio lo tiene que aceptar el dueño** (punto abierto del propio borrador).
2. **Cantidad de posiciones y riesgo total.** La spec (§7.5 y §10) y `config/risk.toml` (`max_open_positions = 5`) permiten **5 posiciones abiertas** como máximo. El borrador puede tener hasta 16 a la vez, cada una con hasta 1 % de pérdida en su stop: en el peor caso, unos 16 % del capital en riesgo al mismo tiempo, frente al 5 % que permite la spec.
3. **Cortos.** La spec permite cortos (§5: «la versión para cortos es el espejo exacto»), así que el borrador no choca con ella. Como dato: en la ronda 2 la tendencia en índices estuvo larga casi siempre y los meses en corto no agregaron nada (`2026-11-ronda2-tendencia-y-momentum-intradia.md`). El benchmark es solo largo.
4. **Timeframe y horarios.** La spec usa 15 minutos de entrada y 1 hora de contexto (§2), cierra todo los viernes (§8), no abre en los primeros 15 minutos de la sesión (§2) y tiene una salida por tiempo a las 8 h. El borrador usa velas diarias, rebalanceo mensual, órdenes en la apertura de la sesión siguiente y posiciones que duran meses. **Hace falta un cambio de spec** (o dejar claro que este programa es solo investigación hasta que se cambie).
5. **Instrumentos.** La spec (§2) solo opera US Tech 100, US 500, Wall Street, Germany 40, Euro Stoxx 50 y acciones del Nasdaq 100; **Nikkei y Hang Seng son "solo contexto"** porque su sesión es de madrugada en España. El oro entró por el ADR 0013. Bonos, divisas, plata, crudo y cobre no están en la spec ni en `config/instruments.toml`, y `config/risk.toml` no tiene tope de apalancamiento para bonos ni para divisas (hoy: índices 10:1, acciones 3:1, oro 10:1, otras materias primas 5:1).
6. **Apalancamiento.** El borrador pone un tope de 4 × el capital al nocional bruto total y reduce todo si el margen pasa del 50 % del capital. La spec limita el apalancamiento **por posición** (§10). Se pueden cumplir los dos, pero hay que decir cuál manda si chocan.
7. **Capital.** La spec usa 1.000 € con aportes de los lunes (§16); el borrador usa 100.000 € y 10.000 €. Con 1.000 € casi ninguna posición llegaría al mínimo de IG.
8. **Slippage.** La spec dice «0,1 × spread promedio + 1 tick» como valor inicial (§11); el borrador usa la mitad del spread. Es más conservador, pero distinto: hay que decidir cuál vale.
9. **Dinero real.** El criterio 6 habla de "pasar a real". Por `CLAUDE.md` §5 y el modo de un solo constructor, pasar a órdenes reales necesita un ADR aprobado por dos personas y hoy está bloqueado. Este programa, aunque pase, queda en simulación.

**Choques con el código y los números del proyecto**

10. **Benchmark "sin costos".** El borrador dice que comprar y mantener **sin costos** da ≈ 1,10 % por mes y Sharpe ≈ 0,90. Esos números son los de la ronda 2 (`2026-11-ronda2-tendencia-y-momentum-intradia.md`, criterio 5: «0,261 (≈ 0,90 anual; +1,10 % por mes)», en EUR), y salen de `engine/crates/backtest/src/research/buy_and_hold.rs`, que **sí paga costos**: spread, slippage y comisión en la entrada y la salida, y la financiación de cada noche menos la base del futuro (#470). Además es comprar y mantener el **CFD de contado** de US500, US100 y US30, no el CFD de futuros. El rótulo "sin costos" no es correcto; el valor sin costos sería otro y no está medido.
11. **El benchmark del borrador no existe en el código.** `buy_and_hold.rs` mantiene el CFD de contado con financiación; el borrador pide mantener los **CFD de futuros** de IG con su spread de futuros y los costos de renovación. Hay que programarlo (y conseguir los spreads de futuros de US 500 y US Tech 100, que IG no publica: §C).
12. **Presupuesto de error.** Al programa 2 le queda **2,5 %** de reserva (ronda 3 gastó 2,5 %: `2026-11-ronda3-volatilidad-y-numeros-redondos.md`). El borrador abre un presupuesto **nuevo** de 5 % para el programa 3 sin decir qué pasa con ese 2,5 %. Si los dos siguen vivos, la tasa total de falsos positivos del proyecto sube. Lo decide el dueño (§D).
13. **Cantidad de pruebas.** El último conteo publicado es **N = 354** (ronda 3). El "al menos 355" del borrador encaja si esta prueba suma una, pero el número exacto se fija al congelar.

**Datos, universo y reglas**

14. **Divisas.** Según IG UK, los CFD de forex no tienen versión de futuros (§C.1). Si IG España lo confirma, por la regla del propio borrador **se caen los 4 mercados de divisas** y la clase de activo entera.
15. **FESX y FGBL.** Databento solo tiene Eurex desde el 2025-03-10 (§B). Por la regla del borrador, también se caen.
16. **Universo resultante y criterio 3.** Si se caen divisas, FESX y FGBL, quedan 10 mercados en **3 clases** (renta variable: ES, NQ, RTY, NKD; tasas: ZN, ZB; materias primas: GC, SI, CL, HG). El criterio 3 pide «al menos 3 de las 4 clases»: con 3 clases habría que redefinirlo **antes** de congelar.
17. **Nikkei.** NKD es el futuro de CME en dólares; el CFD Japón 225 de IG cotiza en USD 5 por punto según su ficha, pero IG no publica sobre qué futuro liquida ni sus vencimientos (§C). Hay que confirmar que NKD es el subyacente correcto.
18. **Renovaciones.** Las fichas de IG muestran que IG vence antes que la bolsa en bonos y materias primas (por ejemplo T-Note: 25-11-2026; oro: 24-11-2026), como dice IG UK («by expiring our contracts slightly earlier»). Para replicar sus fechas desde 2011 hace falta el calendario o la regla de IG (§C, pregunta 4) y probablemente más datos de Databento que el continuo `v.0` (§B).
19. **Velas diarias "a la hora de liquidación".** Con velas de 1 minuto se puede tomar el último precio antes de la hora de liquidación, pero eso **no es** el precio oficial de liquidación (que la bolsa calcula aparte). Hay que definir la hora exacta por mercado en `config/` y decir que es una aproximación.
20. **Stop dentro del día.** Con velas diarias no se sabe si el stop se tocó antes o después de otro evento del día. Como hay velas de 1 minuto, conviene revisar el stop con ellas (y, si hay empate, suponer primero el stop, como la spec §8).
21. **Signo cero.** La regla 2 de la señal da valores −1, −1/3, +1/3, +1, pero no dice qué pasa si un retorno es exactamente 0 (signo 0). Es casi imposible con precios reales, pero una definición objetiva (`CLAUDE.md` §5b) tiene que decirlo.
22. **Conversión de divisa.** IG España publica «0,8 puntos» e IG UK «0.8% fee» (§C.1). Con una cuenta en EUR, casi todos los mercados (todos menos Bund y Euro Stoxx 50) generan ganancias y pérdidas en USD o JPY, así que este costo pesa.
23. **Spreads de futuros.** IG solo publica el spread del futuro de 9 mercados; de los 16 del universo tiene T-Bond (4), oro (0,9 o 0,6: dos páginas dan valores distintos), plata (5,5) y crudo EE. UU. (6). El resto queda a preguntar (§C).

## B. Databento: historia y costo estimado (llamadas gratuitas)

Medido el 24-11-2026 con el SDK oficial (`databento.Historical`) y la clave del `.env`, solo con llamadas de metadatos que no cobran: `metadata.list_datasets`, `metadata.get_dataset_range`, `metadata.get_cost` y `metadata.get_record_count`. **No se descargó nada.** Esquema `ohlcv-1m`, símbolos continuos como en `config/databento.toml` (`stype_in = "continuous"`), desde el 2011-01-01 (o desde el inicio del dataset) hasta el 2026-11-24.

- **Datasets a los que tiene acceso la clave** (los que importan acá): `GLBX.MDP3` (CME Globex: CME, CBOT, NYMEX y COMEX), con `ohlcv-1m` desde el **2010-06-06**; `XEUR.EOBI` (Eurex), con `ohlcv-1m` **solo desde el 2025-03-10**; y los de ICE (`IFEU.IMPACT`, `IFUS.IMPACT`, `NDEX.IMPACT`), **solo desde el 2018-12-23**.
- **Historia desde 2011:** para cada futuro de CME se contaron las velas de 1 minuto del 2011-01-01 al 2011-01-15 con `get_record_count` (`.v.0`): todos tienen velas desde el primer día hábil de 2011 (entre 4.548 y 13.670 velas en esas dos semanas).
- **Ninguno de los 16 mercados está en ICE:** todos son de CME o de Eurex. Los datasets de ICE no ayudan (además empiezan en 2018).
- **FESX y FGBL (Eurex) no tienen historia desde 2011 en Databento:** `XEUR.EOBI` empieza el 2025-03-10. Según la regla del propio borrador («Un mercado se descarta solo si [...] Databento no tiene su historia»), **quedarían fuera**, salvo que el dueño decida otra fuente (decisión 6 de §D).
- **`c.0` o `v.0`:** `c.0` es el vencimiento más cercano en el calendario y `v.0` el de más volumen. En el oro, la plata y el cobre `c.0` casi no tiene operaciones (por ejemplo, SI.c.0 da 109.842 velas contra 5.112.275 de SI.v.0), como ya se había visto con GC en `config/databento.toml`. Para este programa conviene `v.0` en todos (la tabla lo marca).
- **GC ya está cargado (solo lectura en la base):** `futures_bar_loads` tiene GLBX.MDP3 `GC.v.0` del 2011-01-01 al 2026-11-23, 5.512.366 velas, USD 20,12; en `futures_bars_1m` hay 5.512.366 velas de `GC.v.0` (instrumento `XAUUSD`) del 2011-01-02 23:00 UTC al 2026-11-22 23:59 UTC. ES, NQ y RTY también están cargados (ES y NQ desde 2011, RTY desde el 2017-07-09).

| Mercado | Futuro | Dataset | `ohlcv-1m` desde | Velas `c.0` (USD) | Velas `v.0` (USD) | Costo para el programa |
|---|---|---|---|---|---|---|
| S&P 500 | ES | GLBX.MDP3 | 2011 | ya cargado | — | 0 (cargado, USD 20,09) |
| Nasdaq 100 | NQ | GLBX.MDP3 | 2011 | ya cargado | — | 0 (cargado, USD 19,39) |
| Russell 2000 | RTY | GLBX.MDP3 | 2017-07-09 (antes estaba en ICE) | ya cargado | — | 0 (cargado, USD 11,03) |
| Nikkei 225 (USD) | NKD | GLBX.MDP3 | 2011 | 3.540.268 (12,92) | 3.541.490 (12,93) | 12,93 |
| Euro Stoxx 50 | FESX | XEUR.EOBI | **2025-03-10** | 542.987 (14,16) | 542.987 (14,16) | 14,16, pero sin historia desde 2011 |
| Nota EE. UU. 10 años | ZN | GLBX.MDP3 | 2011 | 4.145.020 (15,13) | 5.108.160 (18,65) | 18,65 |
| Bono EE. UU. 30 años | ZB | GLBX.MDP3 | 2011 | 3.792.330 (13,84) | 4.737.913 (17,30) | 17,30 |
| Bund | FGBL | XEUR.EOBI | **2025-03-10** | 519.723 (13,55) | 520.908 (13,58) | 13,58, pero sin historia desde 2011 |
| EUR/USD | 6E | GLBX.MDP3 | 2011 | 3.285.287 (11,99) | 5.459.475 (19,93) | 19,93 |
| USD/JPY | 6J | GLBX.MDP3 | 2011 | 3.169.664 (11,57) | 5.416.109 (19,77) | 19,77 |
| GBP/USD | 6B | GLBX.MDP3 | 2011 | 2.951.964 (10,78) | 5.095.846 (18,60) | 18,60 |
| AUD/USD | 6A | GLBX.MDP3 | 2011 | 3.105.682 (11,34) | 5.344.385 (19,51) | 19,51 |
| Oro | GC | GLBX.MDP3 | 2011 | 241.103 (0,88) | 5.513.746 (20,13) | 0 (ya cargado) |
| Plata | SI | GLBX.MDP3 | 2011 | 109.842 (0,40) | 5.112.275 (18,66) | 18,66 |
| Crudo WTI | CL | GLBX.MDP3 | 2011 | 5.268.166 (19,23) | 5.496.441 (20,07) | 20,07 |
| Cobre | HG | GLBX.MDP3 | 2011 | 165.670 (0,60) | 5.168.969 (18,87) | 18,87 |

**Total estimado** (con `v.0`, lo que falta bajar):
- Los 10 de CME con historia desde 2011 (NKD, ZN, ZB, 6E, 6J, 6B, 6A, SI, CL y HG): **USD 184,30**.
- Si además se bajan FESX y FGBL (solo 2025–2026): USD 212,04.
- Con `c.0` los mismos 10 costarían USD 107,82, pero en plata y cobre casi no hay datos con `c.0`.

**Presupuesto:** `config/databento.toml` pone un tope de USD 100 por corrida, «por debajo de los USD 125 de crédito gratis». Según `futures_bar_loads`, ya se gastaron **USD 100,96** en las cargas hechas (ES, NQ, YM, RTY, GC y FDAX). El saldo real del crédito no sale de estas llamadas: hay que mirarlo en el portal de Databento. **Bajar lo que falta excede el crédito gratis y el tope por corrida, así que es una decisión del dueño** (decisión 4 de §D). Además, el borrador pide renovar en las fechas de IG: con un continuo `v.0` la renovación la decide la regla de Databento (el de más volumen), no la de IG. Para renovar en las fechas de IG haría falta también el contrato siguiente (por ejemplo `v.1`) o los contratos individuales, lo que **aumentaría el costo**; no se estimó porque primero hay que tener el calendario de IG.

## C. IG: qué publica en su web (fuentes públicas, sin la API)

Leído el 24-11-2026, sin usar la API de IG ni gastar cupo. Las fichas de mercado de IG dicen que la información es «solo de referencia» y que los errores no son vinculantes. **Ninguna ficha dice si los datos son del CFD de contado o del de futuros, ni si son del contrato estándar o del mini**: algunas fichas «al contado» (por ejemplo el oro) muestran un vencimiento de futuros. Por eso todo dato de esta tabla queda **para confirmar con IG** antes de congelar. Las horas de vencimiento y de renovación no dicen en qué zona horaria están.

### C.1 Lo general

- **IG España ofrece CFD de futuros sobre índices, materias primas y bonos.** https://www.ig.com/es/nuestros-costes: «podemos operar con CFD de futuros sobre índices y materias primas. Añadimos el coste de financiación nocturna al spread». https://www.ig.com/es/futuros: «Accede a futuros de índices, materias primas y bonos con una sola cuenta», y lista Bobl, Bund, Buxl, Schatz y Gilt largo como futuros de bonos.
- **Divisas: IG no ofrece CFD de futuros (forwards) sobre forex**, según IG Reino Unido. https://www.ig.com/uk/futures-trading/futures-vs-forwards tiene una tabla de mercados CFD con «OTC futures»: *Forex: No* (sí: índices, materias primas, bonos y tasas). Los forwards de forex los ofrece solo como *spread bet* (apuesta financiera, que existe en el Reino Unido y no en España). Pero https://www.ig.com/es/nuestros-costes, en la pregunta frecuente sobre futuros, muestra una tabla con «EUR/USD 10» y «GBP/USD 9». **Hay que preguntarle a IG España** si ofrece CFD de futuros de forex.
- **Renovación** (igual en todas las fichas que la muestran): «Usually, initial position closed at official closing level of day before last dealing day +/- closing spread; new position in next contract opened at official closing level of the new contract from same day, +/- opening spread». Es decir: la renovación paga el spread de cierre y el de apertura, como dice el borrador.
- **Conversión de divisa:** IG España (https://www.ig.com/es/nuestros-costes) dice «El coste estándar de conversión es de 0,8 puntos» (no aclara si son puntos porcentuales), con conversión instantánea por defecto y la opción de convertir diaria, semanal o mensualmente. IG Reino Unido (https://www.ig.com/uk/charges) dice «we'll convert your funds for a 0.8% fee». **A confirmar con IG España:** si es 0,8 % sobre el importe convertido y sobre qué se aplica (ganancias, pérdidas o ambas).
- **Márgenes minoristas por clase** (https://www.ig.com/es/margenes): índices principales 5 %, EUR/USD y GBP/USD 3,33 %, AUD/USD 5 %, oro al contado 5 %, plata, cobre y crudo 10 %. Bonos: 20 % (fichas de bonos y https://www.ig.com/es/futuros).

### C.2 Por mercado

«Mín.» es el tamaño mínimo de la ficha (contratos). «Sin dato» = la página no lo publica. Fichas de IG España salvo que se indique UK.

| Mercado | ¿CFD de futuros en IG? | Spread del futuro | Tamaño de contrato | Mín. | Margen minorista | Vencimiento / última renovación | Fuente |
|---|---|---|---|---|---|---|---|
| S&P 500 (US 500) | Sí (la ficha muestra vencimiento) | Sin dato | USD 250 por punto | 0,05 | 5 % | 18-12-2026 / 18-12-2026 15:15 | https://www.ig.com/es/indices/mercado-indices/us-spx-500 |
| Nasdaq 100 (EE. UU. Tech 100) | Sí (ídem) | Sin dato | USD 100 por punto | 0,05 | 5 % | 18-12-2026 / 18-12-2026 14:15 | https://www.ig.com/es/indices/mercado-indices/eeuu-tech-100 |
| Russell 2000 | Sí en UK (ídem); la ficha de IG España no existe (404) | Sin dato | USD 50 por punto (UK) | 0,01 (UK) | 10 % (UK) | 18-12-2026 / 18-12-2026 14:15 (UK) | https://www.ig.com/uk/indices/markets-indices/russell-2000 |
| Nikkei 225 (Japón 225) | En UK figura entre los futuros de índices; la ficha española no muestra vencimiento | Sin dato | USD 5 por punto | 0,05 | 5 % | Sin dato | https://www.ig.com/es/indices/mercado-indices/japon-225 y https://www.ig.com/uk/futures-trading |
| Euro Stoxx 50 (EU Stocks 50) | En UK figura entre los futuros de índices; la ficha española no muestra vencimiento | Sin dato | EUR 10 por punto | 0,05 | 5 % | Sin dato | https://www.ig.com/es/indices/mercado-indices/eu-stocks-50 y https://www.ig.com/uk/futures-trading |
| Nota EE. UU. 10 años (T-Note decimalizado) | Sí | Sin dato | USD 2 por centavo | 0,05 | 20 % | 25-11-2026 / 25-11-2026 20:45 | https://www.ig.com/es/bonos/mercado-bonos/t-note-decimalizado-10-anos |
| Bono EE. UU. 30 años (T-Bond decimalizado) | Sí | 4 («Bono del Tesoro (Decimalizado)», contrato de 10 $) | USD 10 por centavo | 0,05 | 20 % | 25-11-2026 / 25-11-2026 20:45 | https://www.ig.com/es/bonos/mercado-bonos/t-bond-decimalizado y https://www.ig.com/es/futuros |
| Bund | Sí | Sin dato | EUR 2 por punto | 0,05 | 20 % | 07-12-2026 / 07-12-2026 17:00 | https://www.ig.com/es/bonos/mercado-bonos/bund |
| EUR/USD | No según IG UK; dudoso en IG España (ver C.1) | 10 en la tabla de futuros de la FAQ (sin confirmar) | EUR 100.000 (contado, UK) | 0,04 (contado, UK) | 3,33 % | Sin dato | https://www.ig.com/uk/forex/markets-forex/eur-usd y https://www.ig.com/es/nuestros-costes |
| USD/JPY | No según IG UK | Sin dato | USD 100.000 (contado, UK) | 0,01 (contado, UK) | 3,33 % (UK) | Sin dato | https://www.ig.com/uk/forex/markets-forex/usd-jpy |
| GBP/USD | No según IG UK; dudoso en IG España | 9 en la tabla de futuros de la FAQ (sin confirmar) | GBP 100.000 (contado) | 0,05 (contado) | 3,33 % | Sin dato | https://www.ig.com/es/forex/mercado-forex/forex-contado-gbp-usd |
| AUD/USD | No según IG UK | Sin dato | AUD 100.000 (contado) | 0,05 (contado) | 5 % | Sin dato | https://www.ig.com/es/forex/mercado-forex/forex-contado-aud-usd |
| Oro | Sí | **Dos valores:** 0,9 (https://www.ig.com/es/nuestros-costes, tabla de futuros) y 0,6 (https://www.ig.com/es/futuros) | USD 100 por dólar/onza | 0,05 | 5 % | 24-11-2026 / 24-11-2026 19:15 | https://www.ig.com/es/materias-primas/mercado-materias-primas/oro-al-contado |
| Plata | Sí (figura en la tabla de spreads de futuros) | 5,5 | USD 50 por centavo/onza (ficha de contado 5000 oz) | 0,05 | 10 % | Sin dato (la ficha es de contado) | https://www.ig.com/es/nuestros-costes y https://www.ig.com/es/materias-primas/mercado-materias-primas/plata-contado-5000-oz |
| Crudo WTI (Crudo EE. UU.) | Sí | 6 | USD 10 por centavo/barril | 0,05 | 10 % | 17-12-2026 / 17-12-2026 20:15 | https://www.ig.com/es/materias-primas/mercado-materias-primas/crudo-ligero-eeuu y https://www.ig.com/es/futuros |
| Cobre (Cobre de alto grado) | Sí | Sin dato | USD 2,50 por 0,01 centavo/libra | 0,05 | 10 % | 25-11-2026 / 25-11-2026 18:45 | https://www.ig.com/es/materias-primas/mercado-materias-primas/cobre-de-alto-grado |

Lo que queda **a preguntar a IG** (borrador en `docs/guides/consultas-proveedores.md`, consulta 5):
1. Si IG España ofrece CFD de futuros (forward) de EUR/USD, USD/JPY, GBP/USD y AUD/USD.
2. El spread del CFD de futuros de los 16 mercados (IG publica solo FTSE 100, Wall Street, Alemania 40, Australia 200, oro, plata, crudo EE. UU., Brent y T-Bond), y cuál vale para el oro (0,9 o 0,6).
3. Si los datos de las fichas (tamaño de contrato, mínimo, margen) son del CFD de futuros o del de contado, y del estándar o del mini, y el tamaño mínimo de los mini.
4. El calendario de vencimientos y renovaciones de cada CFD de futuros desde 2011 (o la regla que usan, por ejemplo cuántos días antes del vencimiento de la bolsa), y la zona horaria de las horas publicadas.
5. Si la renovación es automática y si cobra los dos spreads completos.
6. La comisión de conversión: si «0,8 puntos» es 0,8 % y sobre qué se aplica.
7. Russell 2000 en IG España: la ficha española no existe.

## D. Decisiones del dueño

Nada de esto lo decide un agente. Cada respuesta tiene que quedar en el Issue #513 con las palabras del dueño y la fecha.

1. **Criterio 2:** ¿el Sharpe deflactado se aplica a la mezcla 50/50 (como dice hoy el borrador) o a la tendencia sola?
2. **Stop:** ¿el stop de desastre del 1 % del capital cuenta como el stop obligatorio del proyecto (`CLAUDE.md` §5, spec §8)?
3. **Cambio de spec:** ¿se cambia la spec para este programa (timeframe diario, rebalanceo mensual, posiciones de meses sin cierre de los viernes, hasta 16 posiciones a la vez, bonos, divisas y materias primas, Nikkei operable, salidas propias), o el programa queda como investigación hasta que haya un cambio de spec aparte?
4. **Databento:** ¿cuánto se paga? Lo que falta con `v.0` desde 2011 cuesta unos **USD 184** (10 mercados de CME), más de lo que queda del crédito gratis; con FESX y FGBL (solo desde 2025), USD 212. Renovar en las fechas de IG puede costar más (§B).
5. **Presupuesto de error:** ¿el programa 3 reemplaza el 2,5 % que le queda al programa 2 (que se daría por cerrado), o los dos conviven?
6. **FESX y FGBL:** ¿se descartan (regla del borrador, porque Databento no tiene su historia desde 2011) o se busca otra fuente? Si hay otra fuente, hay que nombrarla y, por ser un proveedor de datos nuevo, puede necesitar un ADR (`CLAUDE.md` §6).
7. **Divisas:** si IG España confirma que no hay CFD de futuros de forex, ¿se descartan los 4 mercados (regla del borrador) y cómo queda el criterio 3 con 3 clases de activos?
8. **Benchmark:** ¿se corrige el rótulo "sin costos" del 1,10 % / 0,90 (§A.10) y se programa el comprar y mantener sobre CFD de futuros (§A.11)?
9. **Slippage:** ¿la mitad del spread (borrador) o 0,1 × spread + 1 tick (spec)?
10. **Riesgo total:** ¿se acepta tener hasta 16 posiciones con 1 % de riesgo cada una, o se pone un tope de riesgo total?
11. **Enviar las preguntas a IG** (`docs/guides/consultas-proveedores.md`, consulta 5): solo el dueño las manda.
