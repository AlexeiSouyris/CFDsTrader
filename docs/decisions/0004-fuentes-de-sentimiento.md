# 0004 – Fuentes de sentimiento: vía legal de acceso

- **Estado:** aceptada
- **Fecha:** 2026-11-20 (actualizada el 21-11-2026: enfoque de medidas propias)
- **Aprobada por:** las dos personas, el 21-11-2026 (informado por el dueño)
- **Issue:** #26 (relacionado: #25, #27, ADR 0003 §C.4)

## Contexto

El #25 propone un índice de sentimiento y las hipótesis H1–H7. El ADR 0003 §C.4 y la spec §9 exigen que **cada fuente nueva tenga una vía de acceso legal y documentada**. Para cada una hay que dejar registrado:
- quién la publica y cómo se obtiene;
- su costo y sus límites de uso;
- si sirve para uso privado;
- su hora real de publicación;
- cuánta historia hay.

Para el #25 hace falta historia **desde ~2018**: 3 años de calentamiento más el período de backtest 2021–2026.

Todo lo que sigue se leyó en los **sitios oficiales** el 20-11-2026, con las citas textuales. Lo que no se pudo verificar queda **pendiente**, nunca supuesto.

## Fuentes evaluadas

### 1. CFTC — Commitments of Traders (COT)

| Punto | Dato | Fuente |
|---|---|---|
| Qué es | Posiciones abiertas de futuros y opciones sobre futuros por tipo de operador. El informe **Traders in Financial Futures (TFF)** cubre futuros financieros, incluidos los de índices | [COT, descripción](https://www.cftc.gov/MarketReports/CommitmentsofTraders/index.htm) |
| Hora real | "Generally, the data in the COT reports is from Tuesday and released Friday." "The Commitments of Traders reports are released at 3:30 p.m. Eastern time." Los feriados pueden atrasarlo uno o dos días | [Calendario](https://www.cftc.gov/MarketReports/CommitmentsofTraders/ReleaseSchedule/index.htm) |
| Historia | TFF por año, en texto y Excel: un archivo 2006–2016 y luego uno por año hasta 2026 (`/files/dea/history/fut_fin_txt_AAAA.zip`). **Alcanza** para 2018 | [Históricos](https://www.cftc.gov/MarketReports/CommitmentsofTraders/HistoricalCompressed/index.htm) |
| Costo y términos | Gratis. "Government information at the CFTC website is in the public domain. Public domain information may be freely distributed and copied, but it is requested that in any subsequent use the CFTC be given appropriate acknowledgement." | [Política web](https://www.cftc.gov/webpolicy/index.htm) |
| Acceso automático | Archivos estáticos descargables. El sitio enlaza además un entorno de datos: `publicreporting.cftc.gov`. **No está evaluado**; si se usa, hay que habilitar ese host | Ídem |

**Propuesta: aceptar.** Es dominio público, tiene historia suficiente y la hora de publicación es explícita: el dato del martes se puede usar desde el **viernes a las 15:30 hora del Este**, nunca antes.

**Vía de acceso (actualización del 21-11-2026, decidida por el dueño).** Desde noviembre de 2026, `www.cftc.gov` responde a los pedidos automáticos con un desafío anti-bots ("Just a moment... Enable JavaScript and cookies"), que **no se esquiva**. Se usa la **API pública oficial** de la CFTC: `publicreporting.cftc.gov`, dataset `gpe5-46if`, "TFF - Futures Only". Desde ahí se identifica el sistema como cliente automático, sin datos personales. Hay historia desde 2010 en las series consolidadas del NASDAQ-100 y del S&P 500, y desde 2006 en el E-mini Dow.

Como el calendario oficial de publicación tampoco se puede leer en forma automática, la hora de publicación se toma de manera **conservadora**: el informe del martes vale desde el **martes siguiente a las 15:30 hora del Este** (`config/cot.toml`), o desde que el sistema lo leyó si fue antes. Así cubre los atrasos por feriados.

### 2. Cboe — VIX

| Punto | Dato | Fuente |
|---|---|---|
| Qué es | Cierres diarios del Cboe Volatility Index (VIX) | [VIX, históricos](https://www.cboe.com/tradable_products/vix/vix_historical_data/) |
| Historia | "VIX Index data for 1990 to present (Updated Daily)". CSV enlazado: `cdn-api.cboe.com/api/global/us_indices/daily_prices/VIX_History.csv` | Ídem |
| Exactitud | "furnished without responsibility for accuracy … accuracy is not guaranteed" | Ídem |
| Términos | "You may view, print and download one copy of the Materials for your personal non-commercial use … You may not otherwise copy, reproduce, alter, store either in hard copy or in an electronic retrieval system, license, transmit…" | [Términos de Cboe](https://www.cboe.com/terms) |
| Hora real | No documentada en la página: es un cierre diario. Propuesta: usarlo recién desde el **día hábil siguiente** | — |

### 3. Cboe — Ratio put/call

| Punto | Dato | Fuente |
|---|---|---|
| Qué es | Ratios total, de índices, de acciones, de ETP, del VIX y del SPX, entre otros | [Estadísticas diarias](https://www.cboe.com/us/options/market_statistics/daily/) |
| Historia gratuita | CSV que **terminan el 04-10-2019**: "Cboe Total Exchange Volume and Put/Call Ratios - (11-01-2006 to 10-04-2019)", y lo mismo para índices, acciones y ETP. Después de esa fecha solo está la página del día o **Cboe DataShop (pago)**: "For custom, detailed historical data, visit Cboe DataShop" | [Históricos](https://www.cboe.com/us/options/market_statistics/historical_data/) |
| Términos | Los mismos que el VIX: "Your use of Cboe Volume and Put/Call Ratio data is subject to the Terms and Conditions of Cboe Websites" | Ídem |

**Problema:** de octubre de 2019 a hoy **no hay historia gratuita**, justo el período del backtest. Grabarla día a día desde ahora no alcanza para H1–H3 hasta dentro de 3 años.

### 4. AAII — Encuesta de sentimiento

| Punto | Dato |
|---|---|
| Acceso | **No verificado.** `www.aaii.com` responde a los pedidos automáticos con una protección anti-bots ("Pardon Our Interruption"). No se intentó esquivarla. |
| Consecuencia | La **descarga automática no es viable** sin permiso de AAII. Qué mide, la hora de publicación, cuánta historia hay, si es gratis o solo para socios y los términos de uso tienen que confirmarlos **una persona, en el navegador**, y anotarlos en este ADR. |

### 5. IG — Sentimiento de clientes (% largos)

| Punto | Dato | Fuente |
|---|---|---|
| Acceso | API REST de IG con la cuenta demo. Ya se graba (#27, PR #72): `GET /clientsentiment?marketIds=…`. La referencia dice `/client-sentiment`, pero la demo solo responde a `/clientsentiment` | [Referencia](https://labs.ig.com/reference/client-sentiment.html) |
| Historia | **No hay historial público.** Solo existe lo que grabemos desde ahora | #27 |
| Hora real | La de la consulta. Se guarda en `published_at` | #27 |
| Términos | El acceso es con nuestra clave de API, dentro de los límites de uso de IG (`config/ig.toml`) | ADR 0002 |

### 6. Google Trends (opcional)

No evaluado en este ADR. Necesita su propia revisión de términos y habilitar sus hosts.

### 7. Medidas propias calculadas desde los precios (enfoque FI, 21-11-2026)

El dueño propuso **no pagar fuentes externas** y construir lo nuestro donde se pueda. Los datos crudos no se pueden fabricar: alguien tuvo que registrarlos en su momento. Pero sí se pueden **calcular medidas de miedo y euforia** a partir de los precios que el sistema ya necesita. No hace falta licencia aparte, porque salen de los precios con fuente documentada (IG, ADR 0002, y los del ADR 0005).

| Medida | Definición (a fijar en la spec §9 antes de mirar datos) | Sentido | Datos que necesita |
|---|---|---|---|
| Volatilidad realizada | Desvío de los retornos diarios de los últimos N días, frente a su propia historia | invertido (alta = miedo) | Velas diarias del índice |
| Caída desde el máximo | (cierre − máximo de los últimos N días) / máximo | invertido | Velas diarias del índice |
| Amplitud (*breadth*) | % de las empresas del NASDAQ 100 **de esa fecha** con cierre por encima de su SMA50 | + = euforia | Componentes por fecha (ADR 0005) y sus precios diarios |
| Nuevos máximos y mínimos | (empresas en máximo de 52 semanas − empresas en mínimo de 52 semanas) / total | + = euforia | Ídem |

- **Regla de oro (CLAUDE.md §1):** son indicadores calculados desde precios, así que **se calculan solo en Rust**. Python los lee para armar el índice del #25.
- **Lo que no reemplazan:** el VIX es volatilidad **implícita**, que sale de opciones. No se puede reconstruir sin datos de opciones: la volatilidad realizada es otra medida, parecida pero no igual. Tampoco miden opinión, como la AAII: miden comportamiento del precio.
- **Riesgo de circularidad:** al salir del precio, pueden repetir lo que la estrategia ya ve. Por eso se validan en la fase 5 contra no usarlas, como todo el contexto (ADR 0003 §C), antes de sumar puntos.
- **Historia:** la que tengan los precios. Con velas diarias de 5 años de los índices (ADR 0005) se pueden calcular desde ya las dos primeras. La amplitud y los máximos y mínimos dependen de los precios de los componentes, que siguen pendientes en el ADR 0005.

## Decisión (propuesta, actualizada el 21-11-2026 con el enfoque FI)

**Principio:** fuentes gratuitas con vía legal clara, más medidas propias calculadas por nosotros. No se paga ninguna fuente en v1.

1. **COT (CFTC): aceptar.** Usar TFF, futuros solamente. El dato del martes vale desde el viernes a las 15:30 hora del Este.
2. **IG, sentimiento de clientes: aceptado** (ya en marcha, #27). No hay historia: se graba desde el 20-11-2026. Va **fuera del índice v1** hasta tener 3 años, y sirve para H3 con umbrales absolutos, como propuso el #25.
3. **Medidas propias desde precios (sección 7): aceptar** para el índice v1.
   - Volatilidad realizada y caída desde el máximo: con los índices.
   - Amplitud y nuevos máximos y mínimos: cuando estén los precios de los componentes (ADR 0005).
   - Se calculan en Rust y se definen en la spec §9 **antes** de mirar los datos.
4. **VIX y put/call (Cboe): fuera de v1.** Los términos de Cboe no cubren este uso, y una licencia tiene costo. Se reconsidera solo si las medidas propias no alcanzan en la fase 5.
5. **AAII: fuera de v1.** Su sitio bloquea el acceso automático y no se esquiva.
6. **Índice v1 (#25):** COT + medidas propias desde precios. IG entra cuando tenga historia. La fórmula del percentil sobre 3 años del #25 no cambia: cambian los componentes. El #25 tiene que ajustar su tabla de fuentes y sus hipótesis antes de mirar datos.

## Alternativas consideradas

- **Scraping de sitios que lo prohíben o lo bloquean** (AAII, ratios diarios de Cboe): descartado. Lo prohíbe el #26 y viola los términos.
- **Datos de terceros sin licencia clara** (repositorios o sitios que redistribuyen VIX, put/call o AAII): descartados por la misma razón.
- **Redes sociales:** fuera de v1, como ya decidió el #26.

## Consecuencias

- **Costo cero en fuentes** para el índice v1.
- **Ninguna serie de Cboe ni de AAII** se descarga ni se guarda.
- **Ingesta de la COT:** Python, `intelligence/cfds_intel/sentiment`, según el #25. Puede empezar cuando se apruebe este ADR. Necesita `www.cftc.gov` en la red del entorno, que ya está permitido desde el 20-11-2026.
- **Medidas propias:** son indicadores nuevos en Rust, cada uno con su Issue, su definición en la spec §9 y sus tests de calentamiento y sin repintado.
- **Dónde se guardan:** las observaciones de fuentes externas van a `sentiment_observations` (`contracts/sentiment_observation.v1`, #27), con `observed_for` y `published_at`. Las medidas propias se publican como cualquier indicador del motor.
- **El #25** tiene que ajustar su alcance a lo aprobado acá.
