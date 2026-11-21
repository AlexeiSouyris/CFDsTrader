# 0006 – Fuentes de noticias y calendario económico: oficiales y sin costo

- **Estado:** aceptada
- **Fecha:** 2026-11-21
- **Aprobada por:** las dos personas, el 21-11-2026 (informado por el dueño)
- **Issue:** #106 (relacionado: #99, #91, spec §9, ADR 0004)

## Contexto

La spec §9 pide noticias con su **hora de publicación real** y un **calendario de eventos de alto impacto** para el filtro §7.2 (#91, `contracts/economic_event.v1`). También exige una vía legal documentada para cada fuente y prohíbe el scraping que viole los términos de uso.

Siguiendo el enfoque FI (ADR 0004 §7), en v1 no se paga ninguna fuente: se proponen fuentes **oficiales** y, donde hace falta, una clasificación **propia**.

Todo lo que sigue se leyó en los sitios oficiales el 21-11-2026, con citas textuales. Lo que no se pudo verificar queda **pendiente**.

## Fuentes evaluadas

### 1. Reserva Federal (Board of Governors)

| Punto | Dato | Fuente |
|---|---|---|
| Términos | "Unless otherwise indicated, information on Board's website is in the public domain and may be copied and distributed without permission. Please cite to the Board as the source of the information." | [Disclaimer](https://www.federalreserve.gov/disclaimer.htm) |
| Noticias | Feeds RSS oficiales, entre ellos `press_monetary.xml` (política monetaria), `press_all.xml`, `speeches.xml` y `testimony.xml`. Cada ítem trae `title`, `link` y `pubDate` en GMT. Ejemplo leído: "Federal Reserve issues FOMC statement", `Wed, 28 Oct 2026 18:00:00 GMT` | [Feeds](https://www.federalreserve.gov/feeds/feeds.htm) |
| Calendario | Fechas de las reuniones del FOMC. La Fed publica el calendario por adelantado y a veces lo **revisa**: "Federal Reserve announces revised tentative schedule of Federal Open Market Committee meetings in 2027" (12-11-2026) | [Calendario del FOMC](https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm) |
| Hora real | `pubDate` del feed (GMT) | Ídem |

**Propuesta: aceptar.** Es dominio público y solo pide citar la fuente.

### 2. Banco Central Europeo

| Punto | Dato | Fuente |
|---|---|---|
| Términos | "users of this website may make free use of the information obtained directly from it subject to the following conditions: When such information is distributed or reproduced, it must appear accurately and the ECB must be cited as the source. [...] If the information is modified by the user (e.g. by seasonal adjustment of statistical data or calculation of growth rates) this must be stated explicitly." Hay una excepción para documentos con autores (Working Papers y Occasional Papers), que no se reproducen | [Disclaimer & copyright](https://www.ecb.europa.eu/services/disclaimer/html/index.en.html) |
| Noticias | RSS oficiales: `/rss/press.html` (comunicados, `application/rss+xml`), `/rss/statpress.html` (estadísticas), `/rss/blog.html` y `/rss/pub.html` | [RSS del BCE](https://www.ecb.europa.eu/home/html/rss.en.html) |
| Calendario | Reuniones del Consejo de Gobierno, con las de política monetaria marcadas. Ejemplo leído: "17/12/2026 Governing Council of the ECB: monetary policy meeting in Frankfurt (Day 2), followed by press conference" | [Calendario](https://www.ecb.europa.eu/press/calendars/mgcgc/html/index.en.html) |

**Propuesta: aceptar**, con sus condiciones:
- citar al BCE;
- no alterar la información, y decir explícitamente cuando se la transforma (por ejemplo, una interpretación o un puntaje);
- no reproducir Working Papers ni Occasional Papers.

### 3. Bureau of Labor Statistics (EE. UU.): IPC y empleo

| Punto | Dato |
|---|---|
| Acceso automático | **Rechazado.** Responde 403 a cualquier pedido automático de un cliente que no se identifica: "bot activity that doesn't conform to BLS usage policy is prohibited". |
| Pendiente | Una persona tiene que leer la política de uso del BLS **en el navegador** y anotar acá qué identificación exige. |

**Propuesta: pendiente.** Los datos del BLS son públicos, pero no se accede sin cumplir su política.

### 4. SEC EDGAR

| Punto | Dato |
|---|---|
| Acceso automático | **Rechazado** sin identificación: "Automated access to our sites must comply with SEC.gov's Privacy and Security Policy. Please visit www.sec.gov/developer for more developer resources and Fair Access guidelines." |
| Pendiente | La SEC pide un *User-Agent* con un nombre y un **email de contacto**. Ese email lo eligen las personas: el agente no usa datos personales sin permiso. |

**Propuesta: pendiente** hasta tener un contacto del proyecto. En v1 sirve sobre todo para las empresas del NASDAQ 100, no para los índices.

### 5. Agregadores y medios privados

Forex Factory, Investing.com, Bloomberg, WSJ, FT, El Mundo, TradingView y Yahoo Finance: la spec §9 ya advierte que varios tienen muro de pago o prohíben el acceso automático.

**Propuesta: fuera de v1.** No se evalúan ni se hace scraping. Se reconsideran solo con una vía legal (una licencia), votada por el grupo.

## Decisión (propuesta)

1. **Noticias v1:** los RSS oficiales de la **Fed** y del **BCE**. Cada noticia se guarda con el `pubDate` del feed como `published_at`, el título, el enlace y la fuente. El texto completo no se republica.
2. **Calendario de alto impacto v1:** una **lista propia** en `config/`, armada desde los calendarios oficiales:
   - decisión de tipos del **FOMC** (país `US`): el día de la reunión en que se publica el comunicado, a la hora de publicación de la Fed;
   - decisión de política monetaria del **BCE** (país `EU`): el día 2 de la reunión, a la hora de publicación del BCE;
   - **IPC** y **empleo** de EE. UU. (BLS): cuando se resuelva el punto 3.
3. **Hora de publicación del calendario (anti-look-ahead):** un evento existe para el sistema desde que se publicó el calendario que lo incluye. Si la fuente revisa una fecha, la revisión es un evento **nuevo**, con su propia hora de publicación. En el backtest, para fechas pasadas en las que no se pueda probar cuándo se anunció el calendario, se usa el anuncio documentado. Si no lo hay, el evento queda **fuera**: no se supone que se conocía.
4. **La interpretación de noticias** (sentimiento, relevancia e instrumentos afectados, spec §9) se hace con un **modelo abierto corriendo localmente**, sin pagar ninguna API. Qué modelo, con qué licencia y cómo se evalúa va en otro Issue o ADR.
5. **BLS y SEC:** pendientes de los puntos 3 y 4.

## Alternativas consideradas

- **Calendarios de terceros** (Forex Factory e Investing.com): son cómodos, pero tienen términos restrictivos o prohíben el acceso automático. Descartados.
- **Una API paga de noticias o de calendario:** va contra el enfoque FI de v1. Se reconsidera solo si las fuentes oficiales no alcanzan en la fase 5.
- **Un LLM pago para interpretar:** mismo motivo. Primero se prueba un modelo abierto local.

## Consecuencias

- Hay que habilitar en la red del entorno `www.federalreserve.gov`, `www.bls.gov` y `www.sec.gov`. Ya está hecho desde el 21-11-2026; el BLS y la SEC igual rechazan el acceso hasta que haya identificación.
- El filtro §7.2 (#91) se puede alimentar en v1 con los eventos de FOMC y BCE, todavía sin IPC ni empleo.
- La ingesta de noticias (Python, `intelligence/cfds_intel/news`) empieza cuando se apruebe este ADR, con el contrato `news_item.v1` definido antes (contratos primero).
- Ninguna fuente privada se descarga.
