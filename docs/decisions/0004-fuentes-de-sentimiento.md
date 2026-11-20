# 0004 – Fuentes de sentimiento: vía legal de acceso

- **Estado:** propuesta
- **Fecha:** 2026-11-20
- **Aprobada por:** pendiente (las dos personas)
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

## Decisión (propuesta)

1. **COT (CFTC): aceptar.** Usar TFF, futuros solamente. El dato del martes vale desde el viernes a las 15:30 hora del Este.
2. **VIX y put/call (Cboe): no se cargan hasta que el grupo decida** la vía legal. Los términos de Cboe permiten una copia para uso personal no comercial y prohíben guardarla en un sistema de recuperación electrónico. Una base de datos que alimenta un sistema de trading de un fondo de varias personas probablemente no entra en "uso personal no comercial". Opciones:
   - **a.** Licencia o suscripción de Cboe (DataShop u otra) que cubra este uso: tiene costo.
   - **b.** Pedirle a Cboe una confirmación escrita de que este uso entra en sus términos.
   - **c.** Prescindir de Cboe en el índice v1.

   Para el VIX hay además una alternativa a evaluar: los proveedores de datos de mercado del #10, si su licencia lo incluye.
3. **AAII: pendiente.** Una persona confirma en el navegador el acceso y los términos. Sin eso, AAII no entra.
4. **IG: aceptado** (ya en marcha, #27). Por falta de historia, va **fuera del índice v1** y solo sirve para H3 con umbrales absolutos, como propuso el #25.
5. **Índice v1 (#25):** con lo verificado hoy, la única fuente con historia legal y suficiente es la **COT**. Hasta resolver los puntos 2 y 3, el índice v1 se reduce a COT, o se pospone. Lo deciden las dos personas.

## Alternativas consideradas

- **Scraping de sitios que lo prohíben o lo bloquean** (AAII, ratios diarios de Cboe): descartado. Lo prohíbe el #26 y viola los términos.
- **Datos de terceros sin licencia clara** (repositorios o sitios que redistribuyen VIX, put/call o AAII): descartados por la misma razón.
- **Redes sociales:** fuera de v1, como ya decidió el #26.

## Consecuencias

- Ninguna serie de Cboe ni de AAII se descarga ni se guarda hasta que este ADR se apruebe con la vía elegida.
- La ingesta de la COT (Python, `intelligence/cfds_intel/sentiment`, según el #25) puede empezar cuando se apruebe este ADR. Necesita habilitar `www.cftc.gov` en la red del entorno; ya está permitido desde el 20-11-2026.
- Las observaciones se guardan en `sentiment_observations` (`contracts/sentiment_observation.v1`, #27), con `observed_for` (el martes) y `published_at` (el viernes a las 15:30 hora del Este, en UTC).
- El #25 tiene que ajustar su alcance a las fuentes que efectivamente se aprueben acá.
