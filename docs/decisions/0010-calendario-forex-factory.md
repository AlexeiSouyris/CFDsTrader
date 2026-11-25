# 0010 – Calendario económico de Forex Factory por su feed JSON oficial

- **Estado:** aceptada
- **Fecha:** 2026-11-23
- **Aprobada por:** el dueño (modo de un solo constructor, `CLAUDE.md`), el 23-11-2026. Lo pidió explícitamente con estas palabras: "quiero que descargues e implementes estos git…" y "fijate como adaptarlos a nosotros y que funcionen bien". El pedido se refería a [kjpou1/news_factory](https://github.com/kjpou1/news_factory) y [viandwi24/forexfactory](https://github.com/viandwi24/forexfactory). La aprobación la informó el agente coordinador.
- **Issue:** #238 (ingesta: #239; relacionado: ADR 0006, #91, spec §7.2 y §9)
- **Modificado por:** [ADR 0018](0018-sorpresa-del-calendario.md), solo el punto 7 ("Qué se guarda").
- **Reemplaza:** la parte del [ADR 0006](0006-fuentes-de-noticias.md) que dejó Forex Factory fuera de v1 (§5 "Agregadores y medios privados" y "Alternativas consideradas"), **solo para el calendario económico** y **solo por el feed JSON**. El resto del ADR 0006 sigue vigente.

## Contexto

El filtro de noticias de la spec §7.2 (#91) necesita un calendario de eventos de impacto alto (`contracts/economic_event.v1`). El ADR 0006 lo resolvió en v1 con una lista propia armada desde los calendarios oficiales de la Fed y del BCE. Esa lista no tiene IPC ni empleo de EE. UU.: el BLS rechaza el acceso automático. También queda chica para los demás países.

El dueño pidió usar el calendario de Forex Factory, que cubre todas las monedas principales con su impacto. Se tomaron como inspiración dos repos:
- `news_factory` (licencia MIT): se puede reusar su código o su lista de eventos citando la licencia.
- `viandwi24/forexfactory` (sin licencia): no se copia nada de su código.

Lo que se comprobó el 23-11-2026:

| Punto | Dato |
|---|---|
| Página `https://www.forexfactory.com/calendar` | Responde **403** (Cloudflare) a un cliente automático. Los dos repos la leen con un navegador automatizado. |
| Feed `https://nfs.faireconomy.media/ff_calendar_thisweek.json` | Responde 200 con la semana en curso: unos 75 eventos con `title`, `country` (que en realidad es la **moneda**: USD, EUR...), `date` (ISO 8601 con offset), `impact` (`Low`, `Medium`, `High` o `Holiday`), `forecast` y `previous`. Es el archivo de "Calendar Export" que Forex Factory ofrece para importar el calendario en otras herramientas. |
| `https://nfs.faireconomy.media/ff_calendar_nextweek.json` | Responde **404**: no existe. No se prueban otras URLs. |
| Límite de pedidos | Si se pide de más, responde **429** con `Retry-After` y este texto: "The Calendar export file is only updated once per hour. Requesting it more than that is unnecessary and can result in being blocked." |
| Términos de uso de Forex Factory | No se pudieron leer de forma automática, porque el sitio responde 403. **Pendiente:** el dueño los lee en el navegador y, si prohíben este uso, el ADR vuelve a "propuesta". |

## Decisión

1. **Fuente:** el calendario de Forex Factory se usa **solo por su feed JSON oficial** (`ff_calendar_thisweek.json`), para **uso personal y educativo**. Se cita a Forex Factory como fuente.
2. **Prohibido:**
   - leer la página `forexfactory.com/calendar` o cualquier otra página HTML del sitio (scraping);
   - esquivar la detección de bots o de Cloudflare: nada de Playwright, Selenium, navegadores headless, User-Agent que se haga pasar por un navegador, proxies, rotación de IP ni resolución de captchas;
   - pedir URLs que no estén documentadas o que no respondan.
3. **Identificación honesta:** el cliente se presenta con un User-Agent propio de cliente automático, igual que en `config/news.toml`, sin datos personales.
4. **Frecuencia:** como máximo **un pedido por hora**, porque el archivo se actualiza una vez por hora; lo sugerido es cada pocas horas. Ante un 429 se respeta `Retry-After` y no se reintenta en la misma corrida.
5. **Anti look-ahead** (ADR 0006 §3):
   - El `published_at` de un evento es **la primera vez que lo vimos** en el feed, no la hora del evento.
   - Si Forex Factory cambia la hora o el impacto de un evento, es un evento **nuevo** con su propio `published_at`. El anterior no se borra ni se pisa.
6. **Sin histórico:** el feed trae solo la semana en curso, así que el calendario **se acumula desde el día en que empieza la ingesta**.
   - Para el backtest de fechas anteriores, el filtro §7.2 sigue sin datos de Forex Factory. Vale lo del ADR 0006 §3: sin un anuncio documentado, el evento queda **fuera**.
   - No se baja histórico de ninguna otra fuente para rellenar.
7. **Qué se guarda:** solo lo que pide `economic_event.v1`: el nombre del evento, el país, el impacto, la hora y la hora de publicación. `forecast` y `previous` no se guardan en v1, porque el contrato no los tiene y el filtro no los usa.
   - **Modificado por el [ADR 0018](0018-sorpresa-del-calendario.md) (aceptado el 25-11-2026):** desde ahí `forecast` y `previous` **sí se guardan**, como instantáneas de solo agregar en un contrato y una tabla aparte (`calendar_estimate.v1`, tabla `calendar_estimates`). `economic_event.v1` y el filtro §7.2 no cambian, ni tampoco los puntos 2 a 6 (misma URL, un pedido por hora como máximo, sin scraping).
8. **Código:** la ingesta es propia, en Python (`cfds_intel.calendar`, #239), con la misma salida HTTPS que las noticias (`cfds_intel.net`). Si se porta algo de `news_factory` (por ejemplo su lista de eventos "NNFX"), va como configuración con la atribución MIT.

## Alternativas consideradas

- **Leer la página de Forex Factory con un navegador automatizado** (lo que hacen los dos repos): esquiva la protección del sitio. Prohibido.
- **Quedarse solo con la lista propia del ADR 0006** (FOMC y BCE): es legal y estable, pero no tiene IPC, empleo ni los demás países. Se mantiene como respaldo.
- **Investing.com y otros calendarios:** siguen fuera de v1, por los mismos motivos del ADR 0006.
- **Una API paga de calendario:** va contra el enfoque sin costo de v1 (ADR 0004 §7).

## Consecuencias

- Hay que permitir `nfs.faireconomy.media` en la red del entorno.
- El filtro §7.2 tiene datos de alto impacto de todos los países **desde hoy en adelante**. Para el pasado, el backtest sigue avisando que el filtro no tiene datos.
- Depender de un feed de un tercero sin contrato tiene riesgos: puede cambiar de formato, de URL o de condiciones sin aviso. La ingesta valida cada evento y, si algo no cumple las reglas, falla sin guardar nada. Eso se ve en el log del cron.
- **Pendiente** de que una persona lea los términos de uso de Forex Factory en el navegador. Si no permiten este uso, la ingesta se apaga y este ADR vuelve a "propuesta".
