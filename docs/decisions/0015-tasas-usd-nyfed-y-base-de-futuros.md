# 0015 – Tasas en USD de la NY Fed y base de los futuros en la financiación del backtest

- **Estado:** aceptada
- **Fecha:** 2026-11-24
- **Aprobada por:** el dueño (modo de un solo constructor, `CLAUDE.md`), el 24-11-2026, con las palabras "Yo acepto todas las recomendaciones que me propongas por predeterminado". Esta es la recomendación de #468 (§6), y la aprobación la informó el agente coordinador.
- **Issue:** #467
- **Relacionados:** ADR 0011 (histórico de futuros de Databento), #79 (€STR, `cfds_intel.rates`), #405 y #421 (escenarios de financiación), estudio `docs/analysis/2026-11-doble-conteo-financiacion.md` (#468)

## Contexto

El backtest con `--fuente futuros` usa los precios sin ajustar de los futuros de CME y Eurex (ADR 0011) y les suma la financiación del CFD de IG (referencia ± 3 % de tasa administrativa). Pero, dentro de un contrato, el futuro converge al contado: su deriva ya descuenta la tasa menos los dividendos (`r − q`). El estudio de #468 midió:

- **El doble conteo es la tasa:** en cada noche, a los largos se les cobra `r` de más y a los cortos se les paga `r` de menos. Los dividendos no cuentan doble: según la ayuda internacional de IG, IG abona a los largos y carga a los cortos el ajuste por dividendo en los CFD de índices al contado. Para IG España falta confirmarlo.
- **En la estrategia actual el sesgo es despreciable:** +0,0013 a +0,0018 R por operación.
- **En las tenencias de varios días es decisivo:** en la familia A de #466, ≈ 0,4 R por año de largo en promedio y ≈ 1,0 R en 2022–2025.

Para corregirlo hace falta la tasa de la base noche por noche, con su hora de publicación real. Para el FDAX es el €STR, que ya está en `reference_rates` (#79). Para los futuros de CME (ES, NQ, YM, RTY) es la tasa overnight en USD: la EFFR hasta marzo de 2018 y la SOFR desde su primera publicación. Las dos las administra y publica la Reserva Federal de Nueva York.

| Punto | Dato | Fuente |
|---|---|---|
| API | `https://markets.newyorkfed.org/api/rates/secured/sofr/search.json?startDate=…&endDate=…` y `…/unsecured/effr/search.json`. JSON con `refRates[]`: `effectiveDate`, `type`, `percentRate` | API pública de la NY Fed (probada el 24-11-2026) |
| Publicación | SOFR: "published one business day following its value date", hacia las 8:00 ET. EFFR: "at approximately 9:00 a.m. ET each business day". Revisión posible "at approximately 2:30 p.m. ET" del mismo día, solo si el cambio supera 1 punto básico | [Additional Information about Reference Rates](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates) |
| Términos | "Use of the Reference Rates are subject to important disclaimers, limitations and indemnification obligations" (términos de uso del sitio). Sin costo | [SOFR](https://www.newyorkfed.org/markets/reference-rates/sofr) |

## Decisión

1. **Cargador de la NY Fed** (`cfds_intel.rates.nyfed`), igual al del BCE:
   - EFFR desde 2010-11 y SOFR desde 2018-04-02, a `reference_rates` (`reference_rate.v1`, de solo agregar);
   - cada valor vale desde las **15:00 ET de la fecha del valor siguiente de la serie** (después de la ventana de revisión); el último, desde que se leyó;
   - config en `config/reference_rates.toml` `[nyfed]`; corre con el mismo `python -m cfds_intel.rates`.
2. **Base del futuro en el backtest** (`cfds_backtest::basis`), **solo con velas de futuros**:
   - en cada noche se resta lo que la base ya descontó, `(r − q) × precio × tamaño / 100 / 360`, con el signo del lado;
   - qué serie corresponde a cada futuro va en `config/candle_sources.toml` `[futures.basis]`, por tramos: CME = EFFR → SOFR desde el 02-04-2018; FDAX = €STR;
   - **modo configurable:** `rate` (por omisión, se resta `r`) o `rate_minus_dividends` (se resta `r − q`, con una serie de dividendos con fuente por instrumento, que hoy no existe);
   - **comprar y mantener** sigue la misma regla y además descuenta los saltos de precio de los vencimientos, que no son del mercado;
   - un instrumento sin entrada en `[futures.basis]` no corre con futuros (nunca en silencio). El oro queda para su Issue (#418).
3. Con velas de IG no cambia nada.

## Alternativas consideradas

- **Reconstruir el contado** (`S = F − base`): necesita la previsión de dividendos de cada contrato, que no tenemos con fuente.
- **Base implícita medida con el contrato siguiente** (`c.1` de Databento): es más exacta, pero hay que pagar otra descarga. Queda para después, si la diferencia con la tasa oficial importa.
- **Usar el CSV del estudio (#421) como fuente:** no tiene hora de publicación por valor ni se actualiza. Se descarta para lo oficial.
- **Tasa constante en config:** inventaría valores. Se descarta.

## Consecuencias

- El backtest con futuros **necesita EFFR y SOFR cargadas** (`python -m cfds_intel.rates`). Si faltan, frena con un error que dice cómo cargarlas.
- Los resultados con futuros de las tenencias de varios días (familia A, D1 y E de #466) quedan sin el doble conteo. En la estrategia actual cambian ≈ 0,0015 R por operación.
- Comprar y mantener con futuros cambia un poco: sin saltos de vencimiento y sin la tasa doble, se acerca a lo que daría el CFD.
- **Pendiente:** confirmar con IG España que ajusta los dividendos en los CFD de índices al contado. Si no los ajusta, se pasa a `rate_minus_dividends` y hace falta una serie de dividendos con fuente.
