# 0011 – Histórico intradía de futuros desde Databento

- **Estado:** aceptada el 23-11-2026 por el dueño ("si si empeza a descargar los datos dale", en la sesión de su agente, después de ver el costo de USD 30,45 del crédito gratis)
- **Fecha:** 2026-11-23
- **Aprobada por:** pendiente (el dueño; modo de un solo constructor, `CLAUDE.md`)
- **Issue:** #246 (importador: #247; verificación: #248; relacionado: ADR 0002, ADR 0005, ADR 0009, #132)

## Contexto

El backtest necesita años de velas de 15m y 1h, y el cupo de IG no alcanza para bajarlas (#132). En IG, cada vela es un punto del cupo semanal. Tiingo (ADR 0009) solo sirve para verificar diarias de ETF.

El dueño pidió preparar **Databento**, que vende datos de bolsa con precios por uso. Todavía no hay cuenta, así que este ADR y el importador se preparan **sin key y sin gastar**.

Se leyó la documentación oficial de Databento el 23-11-2026 (`databento.com/docs`). Esto es lo que se encontró:

| Punto | Dato | Fuente |
|---|---|---|
| Crédito gratis | "New users get $125 in free credits" (vence a los 6 meses, según la FAQ) | [GLBX.MDP3](https://databento.com/datasets/GLBX.MDP3) y [FAQ de uso y créditos](https://databento.com/docs/faqs/usage-pricing-and-data-credits) |
| Cómo se cobra | El histórico se cobra por GB, medido sin comprimir: "We meter the data by its uncompressed size in binary encoding." | [Basics](https://databento.com/docs/api-reference-historical/basics) |
| Metadata gratis | "Access to metadata, symbology, and account management is free." También: "You will only be billed for usage of time series data." | [Basics](https://databento.com/docs/api-reference-historical/basics) |
| Precio real (medido) | Medido el 23-11-2026 con la key del dueño, solo con metadata (`get_cost` y `get_billable_size`, sin bajar datos): unos **USD 65/GB en CME** y unos **USD 465/GB en Eurex** para `ohlcv-1m`. Los USD 0,50 y USD 5 por GB de la web son precios "desde", no los de este esquema. Ver la tabla de costos abajo | metadata de Databento (cuenta del dueño) |
| CME (`GLBX.MDP3`) | "full coverage of all futures and options on futures beginning in June 2010" | [GLBX.MDP3](https://databento.com/datasets/GLBX.MDP3) |
| Eurex (`XEUR.EOBI`) | La página no dice desde cuándo hay histórico. Medido con `get_dataset_range` el 23-11-2026: **arranca el 2025-03-10**. `FDAX.c.0` existe y tiene `ohlcv-1m` | [XEUR.EOBI](https://databento.com/docs/datasets/xeur-eobi) y metadata de la cuenta |
| Vela OHLCV | `ts_event` es "The inclusive start time of the bar aggregation period", en nanosegundos UTC. Los precios son enteros con escala 1e-9. "If no trade occurs within the interval, no record is printed." | [OHLCV](https://databento.com/docs/schemas-and-data-formats/ohlcv) |
| Contrato continuo | `RAÍZ.REGLA.RANGO` (por ejemplo `ES.c.0`): `c` = calendario, `n` = interés abierto, `v` = volumen del día anterior. "The continuous contract prices returned are the original, unadjusted prices." | [Symbology](https://databento.com/docs/standards-and-conventions/symbology) |
| Límites de pedidos | "20 metadata requests per second", "100 time series requests per second", y 429 con `Retry-After` | [Basics](https://databento.com/docs/api-reference-historical/basics) |
| Key | Si no se pasa, el cliente usa la variable `DATABENTO_API_KEY` | [Client](https://databento.com/docs/api-reference-historical/client) |
| **Licencia** para uso personal no profesional de datos históricos | **No encontrada.** `docs/faqs/licensing`, `/terms` y `/licensing` dan 404, y `/legal` no tiene texto. Solo hay una frase sobre las licencias de los datos **en vivo**: "We pass through these license fees with no upcharge." | — |

### Costo medido (23-11-2026)

Medido con `get_cost` y `get_billable_size`, `ohlcv-1m`, contrato continuo `c.0`, hasta el 2026-11-21. No se bajó nada.

| Símbolo | Desde | Costo | Tamaño |
|---|---|---|---|
| `ES.c.0` | 2021-11-01 | USD 6,4969 | 99,7 MB |
| `NQ.c.0` | 2021-11-01 | USD 6,4851 | 99,5 MB |
| `YM.c.0` | 2021-11-01 | USD 6,3271 | 97,1 MB |
| `FDAX.c.0` | 2025-03-10 (inicio del dataset) | USD 11,1373 | 23,9 MB |
| **Total** | | **USD 30,45** | |

Los **USD 125 de crédito gratis cubren el total**, y queda margen para repetir algo o sumar meses.

## Decisión (propuesta)

1. **Fuente:** Databento, solo **histórico**, con el SDK oficial de Python (`databento`, versión 0.90 al 23-11-2026). Se usan:
   - CME Globex (`GLBX.MDP3`): **ES**, **NQ** y **YM**;
   - Eurex (`XEUR.EOBI`): **FDAX**;
   - el esquema **`ohlcv-1m`**, desde **2021-11-01**, o desde el inicio del dataset si es posterior. El inicio sale de `get_dataset_range` en cada corrida, no está fijo en el código.
   - **DE40 tiene 15m de Databento solo desde 2025-03** (Eurex arranca el 2025-03-10). Lo anterior de DE40 sale de IG, con el cupo.
2. **Mapeo** a nuestros instrumentos, por contrato continuo de calendario (`c.0`, el más cercano al vencimiento):

   | Futuro | Contrato continuo | Instrumento (CFD de IG) |
   |---|---|---|
   | ES (E-mini S&P 500) | `ES.c.0` | US500 |
   | NQ (E-mini Nasdaq-100) | `NQ.c.0` | US100 |
   | YM (E-mini Dow) | `YM.c.0` | US30 |
   | FDAX (futuro del DAX) | `FDAX.c.0` | DE40 |

3. **Vencimientos (rollover):**
   - Se usa **`c.0`**: el contrato con el vencimiento más cercano, que pasa al siguiente en el vencimiento.
   - Por qué `c.0` y no `v.0` (el de más volumen):
     - El CFD de IG sigue al índice de **contado**, y la base del futuro baja a cero a medida que se acerca el vencimiento. Por eso el contrato más cercano es el que más se parece al CFD.
     - El costo ya está **medido** con `c.0` (la tabla de arriba).
     - El costo depende de las barras que se bajan, y `v.0` también trae una barra por minuto operado, así que no debería cambiar mucho. **No está medido:** si se prefiere `v.0`, hay que medirlo antes con `--estimate`, que usa solo metadata.
   - Lo que se pierde: en los últimos días antes del vencimiento, el contrato cercano opera menos. Puede haber más minutos sin barra, y la verificación (#248) los va a mostrar como días con menos barras.
   - Los precios **no se ajustan** (Databento no lo hace y nosotros tampoco).
   - Cada barra se guarda con el `instrument_id` del contrato real. Un cambio de `instrument_id` marca el rollover, y ninguna vela agrupada mezcla dos contratos.
   - Lo que se mide entre contratos (retornos, base) se calcula **dentro de cada contrato**. El salto del rollover no es un movimiento del mercado.
4. **Futuro contra CFD (la base):**
   - El CFD de IG sigue al índice de contado, y el futuro cotiza con una base (tipos y dividendos hasta el vencimiento) que cambia con cada contrato.
   - La base se **calibra** con las velas de IG que se solapen: diferencia de cierres por contrato, con su media y su desvío (verificación, #248).
   - **El spread, la comisión, el slippage y la financiación siguen siendo los de IG** (`config/instruments.toml`). Databento solo da la forma del precio.
5. **Agrupación en Rust:**
   - Las barras de 1m se agrupan en velas de 15m y 1h (y diarias para verificar) **en Rust**, con la misma regla de velas cerradas que las de IG.
   - Python solo baja y guarda las barras de 1m tal como llegan, como hace con otras fuentes crudas (COT, €STR). Así ningún cálculo sobre el precio vive en dos lugares.
6. **Solo el libro central:**
   - En Eurex, OHLCV trae **dos registros para el mismo minuto** cuando hubo operaciones dentro y fuera del libro: "two OHLCV messages will be published for that instrument on that interval, one with `XEUR.EOBI.XEUR` as the `publisher_id` and one with `XEUR.EOBI.XOFF`" ([XEUR.EOBI](https://databento.com/docs/datasets/xeur-eobi)).
   - Las operaciones fuera de libro (TES, "Trades reported in `TES Trade Report` EOBI message are considered off-market") se pactan a precios que no son del mercado.
   - Se guarda solo el publisher del libro central, que se configura por símbolo (`publisher` en `config/databento.toml`): `XEUR.EOBI.XEUR` (101) en Eurex y `GLBX.MDP3.GLBX` (1) en CME, que tiene un solo publisher. El resto se descarta y se cuenta (#254, agregado el 23-11-2026).
7. **Guardado:**
   - Las barras van en una tabla **nueva**, de solo agregar (`futures_bars_1m`, migración 0023), con su fuente, dataset, símbolo continuo, contrato y la hora en UTC.
   - **No se tocan las velas de IG** (`candles` no distingue fuentes).
8. **Costo:**
   - Antes de bajar nada, `--estimate` pide `metadata.get_cost` y `metadata.get_record_count` (gratis según la doc: "Access to metadata ... is free") por símbolo.
   - **Frena** si el total pasa `max_usd` de `config/databento.toml` (100, por debajo de los 125 de crédito).
   - `--run` vuelve a estimar y frena igual.
9. **Datos solo en el Mac:** las barras viven solo en la base local de la máquina de datos (`CFDS_DATA_MACHINE=1`). **Nunca en el repo**: ni precios ni archivos DBN, y los tests usan respuestas sintéticas.
10. **Key:** sale de `DATABENTO_API_KEY` (`.env`, con un valor falso en `.env.example`) y nunca se imprime.

## Preguntas abiertas (antes de aceptar)

1. **Licencia:** ¿qué permite Databento para un usuario personal, no profesional, con datos históricos de CME y Eurex? No se encontró la cláusula en la documentación pública. Hay que leer los términos al crear la cuenta y citarlos acá.
2. **Confirmar el gasto:** USD 30,45 de los 125 de crédito (tabla de arriba). Hasta que el dueño lo confirme no se corre `--sample` ni `--run`.
3. **Regla de rollover:** se propone `c.0`. Si la verificación (#248) muestra que los días cerca del vencimiento empeoran la comparación con IG, se mide `v.0` con `--estimate` y se cambia en `config/databento.toml`.

## Alternativas consideradas

- **Bajar el histórico de IG:** cuesta un punto por vela del cupo semanal. Años de 15m de cuatro índices llevan meses de cupo (#132).
- **Tiingo** (ADR 0009): no tiene intradía de futuros ni de índices en el plan gratis.
- **Bajar desde Rust:** en las páginas oficiales leídas, la API HTTP no aparece documentada de forma legible; solo los métodos del SDK de Python. No se inventan endpoints (`CLAUDE.md` §5b). Además, Python ya baja las otras fuentes crudas. Rust queda para agrupar y verificar.
- **Ajustar los precios por el rollover:** cambia los niveles (pivotes, máximos iguales) que el motor compara con el precio del CFD. Se prefiere no ajustar y medir dentro de cada contrato.

## Consecuencias

- Hay que permitir `hist.databento.com` en la red de la máquina de datos.
- `intelligence/` suma la dependencia `databento`, que trae pandas, numpy y pyarrow.
- El backtest va a poder usar años de 15m y 1h con la forma del precio del futuro y los costos de IG, pero con la **base** como fuente de error conocida. Por eso la verificación contra IG (#248) es obligatoria antes de usarlo.
- DE40 tiene histórico de futuros solo desde el 2025-03-10. Para antes, el backtest de DE40 depende de lo que se baje de IG.
- Hasta que el dueño responda las preguntas abiertas y confirme el gasto, nada se baja.
