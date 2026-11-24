# 0016 – Calendario histórico oficial de FOMC, CPI y empleo (Fed y BLS), 2011–2025

- **Estado:** propuesta
- **Fecha:** 2026-11-24
- **Aprobada por:** pendiente. La aprueba el dueño (modo de un solo constructor, `CLAUDE.md`), con su aprobación explícita registrada en el Issue o en el PR. Este ADR no habilita dinero real.
- **Issue:** #505 (la ingesta va en otro Issue, después de aprobar este)
- **Relacionados:** [ADR 0006](0006-fuentes-de-noticias.md) (§1 Fed, §3 BLS, regla de publicación del calendario), [ADR 0010](0010-calendario-forex-factory.md) (Forex Factory, solo desde 2026-11-23), [ADR 0012](0012-tesoro-y-vix.md) (ejemplo de fuente oficial), `docs/analysis/2026-11-programa-2-propuesta.md` §1.1 b) y familia E, #466 (D2)

## Contexto

El programa 2 (#493) dejó la **familia E** (eventos programados: la deriva antes del anuncio de la Fed, Lucca y Moench 2015; los días de CPI y empleo, Savor y Wilson 2013) en reserva porque falta el calendario histórico. La tabla `economic_events` solo tiene Forex Factory **desde la semana del 23-11-2026**: su feed no trae historia (ADR 0010 §6), y ese ADR prohíbe rellenar el pasado con otra fuente **para el filtro §7.2**. Este ADR propone una fuente **distinta y oficial** para el pasado, con su propio uso: el estudio de eventos de la familia E y, si el dueño lo decide, el filtro §7.2 en el backtest.

Hace falta, para 2011–2025:

- la fecha y hora de cada **comunicado del FOMC** (reuniones programadas y no programadas);
- la fecha y hora de cada publicación del **CPI** y del **informe de empleo** (*Employment Situation*) del BLS;
- **desde cuándo se sabía** cada fecha (`CLAUDE.md` §5b y ADR 0006 §3).

Son unas 140 filas de la Fed y unas 360 del BLS. Es un dato chico, público y sin costo.

Lo que se comprobó el 24-11-2026 (WebFetch y WebSearch; `curl` con un User-Agent propio para los códigos HTTP):

### Reserva Federal (federalreserve.gov)

| Punto | Dato | Fuente |
|---|---|---|
| Calendario 2021–2027 | "Meeting calendars, statements, and minutes (2021-2027)". Ocho reuniones programadas por año "and other meetings as needed". Las fechas futuras son "tentative until confirmed at the meeting immediately preceding it". No da horas | [fomccalendars.htm](https://www.federalreserve.gov/monetarypolicy/fomccalendars.htm) |
| Calendario 2011–2020 | Una página por año, `https://www.federalreserve.gov/monetarypolicy/fomchistorical{AAAA}.htm` (por ejemplo [2013](https://www.federalreserve.gov/monetarypolicy/fomchistorical2013.htm), que responde 200). Marca cada reunión: programada, *unscheduled*, *cancelled* o *notation vote*. Los años pasan a "históricos" con unos cinco años de atraso | [fomc_historical.htm](https://www.federalreserve.gov/monetarypolicy/fomc_historical.htm), [selector por año](https://www.federalreserve.gov/monetarypolicy/fomc_historical_year.htm) |
| Comunicado de cada reunión | Página fija `https://www.federalreserve.gov/newsevents/pressreleases/monetary{AAAAMMDD}a.htm`. Desde 2020 trae la hora: "For release at 2:00 p.m. EST" ([18-12-2024](https://www.federalreserve.gov/newsevents/pressreleases/monetary20241218a.htm)). En 2013 dice solo "For immediate release", **sin hora** ([30-1-2013](https://www.federalreserve.gov/newsevents/pressreleases/monetary20130130a.htm), [20-3-2013](https://www.federalreserve.gov/newsevents/pressreleases/monetary20130320a.htm)) | Ídem |
| Anuncio del calendario | La Fed publica el calendario tentativo de cada año por comunicado, meses antes. Ejemplos: el de 2013 (con el de 2012 revisado), el [16-5-2012](https://www.federalreserve.gov/newsevents/pressreleases/monetary20120516b.htm); el de 2014, el [5-4-2013](https://www.federalreserve.gov/newsevents/pressreleases/monetary20130405a.htm); el de 2015, el [5-6-2014](https://www.federalreserve.gov/newsevents/pressreleases/monetary20140605b.htm); una revisión del de 2027, el [12-11-2026](https://www.federalreserve.gov/newsevents/pressreleases/monetary20261112a.htm). **No se verificó** uno por cada año 2011–2025: lo hace la ingesta | Comunicados citados |
| API o archivo descargable | **No hay** API ni CSV oficial del calendario del FOMC. Los feeds RSS (ADR 0006 §1) traen solo los comunicados recientes | [Feeds](https://www.federalreserve.gov/feeds/feeds.htm) |
| `robots.txt` | `https://www.federalreserve.gov/robots.txt` responde **404**: el sitio no publica reglas para robots | Comprobado con `curl` |
| Términos | "Unless otherwise indicated, information on Board's website is in the public domain and may be copied and distributed without permission." Piden: "Please cite to the Board as the source of the information." No dicen nada de acceso automático ni de límites de pedidos | [Disclaimer](https://www.federalreserve.gov/disclaimer.htm) |

### Bureau of Labor Statistics (bls.gov)

| Punto | Dato | Fuente |
|---|---|---|
| Calendario por año | `https://www.bls.gov/schedule/{AAAA}/home.htm`, de 2000 a 2026 (1957–2000 en un PDF). "All times on calendar are Eastern Time." CPI y empleo figuran a las **08:30 AM** con su mes de referencia (comprobado en [2013](https://www.bls.gov/schedule/2013/home.htm) y [2025](https://www.bls.gov/schedule/2025/home.htm)) | [Prior Years](https://www.bls.gov/bls/archived_sched.htm) |
| Calendario por serie | Solo el año en curso y el siguiente: [CPI](https://www.bls.gov/schedule/news_release/cpi.htm) y [Employment Situation](https://www.bls.gov/schedule/news_release/empsit.htm) | — |
| iCalendar | `https://www.bls.gov/schedule/news_release/bls.ics`: solo los "upcoming months". **No sirve para el histórico** | [Subscribe to iCal Calendar](https://www.bls.gov/help/hlpical.htm) |
| Lo que muestran los años pasados | Las páginas por año muestran la fecha **en que finalmente se publicó**, sin marcar las reprogramaciones. Ejemplo: el empleo de septiembre de 2013 figura el martes 22-10-2013, y estaba previsto para el viernes 4-10-2013 | [2013](https://www.bls.gov/schedule/2013/home.htm) y avisos de abajo |
| Aviso de la fecha siguiente | Cada informe dice cuándo sale el próximo: "The Employment Situation for October is scheduled to be released on Friday, November 8, 2013, at 8:30 a.m. (EST)." También trae la hora de embargo: "embargoed until 8:30 a.m. (EDT) Tuesday, October 22, 2013" | [Empleo, 22-10-2013](https://www.bls.gov/news.release/archives/empsit_10222013.htm) |
| Reprogramaciones por cierres | 2013: cierre del 1 al 16-10-2013; empleo de septiembre del 4-10 al 22-10, empleo de octubre del 1-11 al 8-11 | [Updated Schedule of BLS News Releases](https://www.bls.gov/bls/updated_release_schedule.htm), [blog del BLS, 2013](https://www.bls.gov/blog/2013/adjusted-news-release-schedule-new-monthly-labor-review-article-and-women-s-earnings-report.htm) |
| | 2025: cierre del 1-10 al 12-11-2025; empleo de septiembre del 3-10 al 20-11; **empleo de octubre cancelado** (sus datos de establecimientos salen con los de noviembre); CPI de septiembre publicado el 24-10; **CPI de octubre cancelado**; CPI de noviembre del 10-12 al 18-12 | [Revised news release dates following the 2025 and 2026 lapses in appropriations](https://www.bls.gov/bls/2025-lapse-revised-release-dates.htm), [CPI y el cierre de 2025](https://www.bls.gov/cpi/additional-resources/2025-federal-government-shutdown-impact-cpi-faq.htm) |
| API oficial | La [Public Data API](https://www.bls.gov/bls/api_features.htm) da **valores** de series, no fechas ni horas de publicación. No sirve para el calendario | — |
| Acceso automático | Un pedido con un User-Agent propio **sin datos de contacto** responde **403**: "bot activity that doesn't conform to BLS usage policy is prohibited". Pasa también con `https://www.bls.gov/robots.txt`, así que **no se pudo leer el `robots.txt` del BLS**. Las páginas sí cargaron por WebFetch | Comprobado con `curl` |
| Política de uso | "excessive robot activity on BLS websites is prohibited"; "BLS will block robots that access the website in any way that BLS considers excessive or malicious, including robots that attempt to access or download survey information multiple times per second"; "BLS also reserves the right to block robots that do not contain information that can be used to contact the owner." No dice cómo identificarse (no menciona el User-Agent) | [Protecting Online Privacy](https://www.bls.gov/bls/pss.htm) |
| Términos | "everything that we publish, both in hard copy and electronically, is in the public domain", "You are free to use our public domain material without specific permission, although we do ask that you cite the Bureau of Labor Statistics as the source." | [Copyright Information](https://www.bls.gov/opub/copyright-information.htm) |

Esto resuelve en parte el "pendiente" del ADR 0006 §3: la política del BLS ya está leída. Exige un robot que no sea excesivo y que traiga **datos de contacto del dueño**.

### Horas de publicación del comunicado del FOMC (todas en hora del Este, ET)

| Período | Hora | Fuente |
|---|---|---|
| Enero 2011 hasta la reunión de marzo de 2011 | **2:15 p.m.** | "From 1999 to 2010, with a few exceptions, these statements were announced at 2:15 PM Eastern Time" (Byun, Fees, Jacobson y Walker, [FEDS 2026-029](https://www.federalreserve.gov/econres/feds/files/2026029pap.pdf)) |
| Abril 2011 a enero 2013, reuniones **con conferencia de prensa** (y proyecciones) | **~12:30 p.m.** | Comunicado del [24-3-2011](https://www.federalreserve.gov/newsevents/pressreleases/monetary20110324a.htm): "For these meetings, the FOMC statement is expected to be released at around 12:30 p.m., one hour and forty-five minutes earlier than for other FOMC meetings". FEDS 2026-029: "For the two years from 2011 to 2012, if a post-meeting press conference occurred, the FOMC issued a press release at 12:30 PM Eastern Time" |
| Abril 2011 a enero 2013, reuniones **sin** conferencia | **2:15 p.m.** | FEDS 2026-029: "for meetings without press conferences, the Fed maintained the 2:15 PM release time". [FEDS 2025-048](https://www.federalreserve.gov/econres/feds/files/2025048pap.pdf): "Prior to the March 2013 FOMC meeting, the statement was typically released at or around 2:15 p.m." |
| Desde la reunión del 19–20-3-2013 | **2:00 p.m.** | Comunicado del [13-3-2013](https://www.federalreserve.gov/newsevents/pressreleases/monetary20130313a.htm): "Committee policy statements for all regularly scheduled meetings will now be released at 2 p.m. Eastern Time"; la conferencia, "at approximately 2:30 p.m." |
| Reuniones no programadas | La hora de **cada** comunicado: 3-3-2020 "For release at 10:00 a.m. EST"; 15-3-2020 (domingo) "For release at 5:00 p.m. EDT"; 23-3-2020 (*notation vote*) "For release at 8:00 a.m. EDT" | [3-3-2020](https://www.federalreserve.gov/newsevents/pressreleases/monetary20200303a.htm), [15-3-2020](https://www.federalreserve.gov/newsevents/pressreleases/monetary20200315a.htm), [23-3-2020](https://www.federalreserve.gov/newsevents/pressreleases/monetary20200323a.htm), [2020](https://www.federalreserve.gov/monetarypolicy/fomchistorical2020.htm) |

El "around 12:30 p.m." y el "typically ... at or around 2:15 p.m." no son horas exactas. La hora de cada comunicado anterior a 2020 se toma de esta regla, porque la página del comunicado no la trae (ver Riesgos).

## Decisión (propuesta)

### 1. Fuentes y forma de acceso

1. **Solo las páginas oficiales citadas arriba.** Nada de terceros que las copien (Forex Factory, Investing.com, repositorios de GitHub, Wikipedia).
2. **Fed:** lectura automática de las páginas por año (`fomchistorical{AAAA}.htm` para 2011–2020, `fomccalendars.htm` para 2021–2025), de cada comunicado (`monetary{AAAAMMDD}a.htm`) y de los comunicados del calendario tentativo. Son datos de dominio público, sin `robots.txt` y sin límites publicados. Aun así: User-Agent propio de cliente automático (como `config/news.toml`), **un pedido por vez con al menos 1 s entre pedidos**, y la carga se corre **una sola vez** (son unas 160 páginas).
3. **BLS: sin robot.** La política exige datos de contacto del dueño en el robot, y eso es un dato personal que el proyecto no pone en el código. Como son 15 páginas por año más los avisos de cierres, **el dueño las guarda a mano desde el navegador** (o se las da a un agente que las lea con su herramienta web, no con un script) y el parser lee los HTML guardados en local. Si el dueño prefiere un robot, la alternativa es un User-Agent con un contacto que el dueño ponga en `.env` (nunca en el repo), un pedido cada pocos segundos y freno ante el primer 403. **Lo elige el dueño al aprobar.**
4. **Prohibido**, igual que en el ADR 0010: esquivar el 403 del BLS (navegadores automatizados, User-Agent que se haga pasar por un navegador, proxies, rotación de IP) y pedir URLs que no estén documentadas acá.
5. **Se cita la fuente** ("Board of Governors of the Federal Reserve System", "U.S. Bureau of Labor Statistics"), como piden los dos términos.

### 2. Qué eventos entran

| Serie | Evento | Hora (ET) |
|---|---|---|
| `fomc_statement` | Cada comunicado del FOMC tras una reunión programada, 2011–2025 | Según la tabla de horas de arriba |
| `fomc_statement` (no programado) | Cada comunicado de una reunión *unscheduled* o *notation vote* que publicó un comunicado (por ejemplo 3, 15 y 23-3-2020). Las reuniones sin comunicado (por ejemplo la conferencia del 16-10-2013) **no entran** | La del comunicado ("For release at ...") |
| `cpi` | Cada publicación del CPI, con su mes de referencia | 08:30 |
| `employment_situation` | Cada informe de empleo, con su mes de referencia | 08:30 |

No entran las actas del FOMC, las proyecciones aparte del comunicado, los discursos ni otros datos del BLS: no los pide la familia E. Se agregan con otro Issue si hacen falta.

### 3. Hora y zona horaria

1. Cada hora se escribe en **hora del Este** como dice la fuente, y se convierte a UTC con la base de zonas horarias (`America/New_York`, `zoneinfo`), **nunca con un desfase fijo**. Así 08:30 ET es 13:30 UTC en invierno (EST) y 12:30 UTC en verano (EDT). Los tests cubren los días de cambio de hora, por ejemplo un CPI de marzo después del segundo domingo.
2. La regla de horas del FOMC (2:15, ~12:30, 2:00) va en `config/`, con la fecha de cada cambio y su fuente, no en el código (`CLAUDE.md` §5b).
3. Si la página de un comunicado trae "For release at ...", esa hora **gana** sobre la regla. Si se contradicen, la ingesta falla en vez de elegir en silencio.

### 4. Desde cuándo vale cada fila (`published_at`, anti look-ahead)

Es la regla del ADR 0006 §3 aplicada al histórico: **un evento existe para el sistema desde que se publicó el calendario que lo incluye**.

- **FOMC programado:** `published_at` = la fecha y hora del comunicado que anunció el calendario tentativo de ese año, o el de la revisión que fijó esa fecha. Si el comunicado no trae hora, se toma el **final de ese día** (23:59:59 ET), del lado seguro.
- **CPI y empleo programados:** `published_at` = la hora de embargo del **informe anterior de la misma serie**, que anuncia la fecha siguiente (por ejemplo, el empleo del 22-10-2013 anuncia el del 8-11-2013). Es conservador: el calendario anual del BLS sale antes, pero no se pudo documentar cuándo.
- **Reprogramados (cierres de 2013 y 2025):** la fila original **no se borra**. La nueva fecha es otra fila con `published_at` = la fecha del aviso del BLS que la anunció, si se puede documentar. Si no, vale la regla del ADR 0006 §3: la fila queda **fuera** y no se supone que se conocía. Las publicaciones canceladas (empleo y CPI de octubre de 2025) quedan como cancelaciones con su `published_at`.
- **FOMC no programado:** `published_at` = la hora del comunicado. No se sabía antes. **No sirve** para estudiar lo que pasa *antes* del anuncio.
- Si para una fila no hay forma documentada de saber cuándo se conoció, **queda fuera**.

Estos datos dicen **cuándo** hubo un anuncio, no **qué** dijo. Un estudio que use la sorpresa del dato (CPI contra lo esperado) necesita otra fuente y otro ADR.

### 5. Cómo se guardaría (solo diseño; no se implementa en este PR)

**Contrato nuevo `contracts/official_release.v1`** y **tabla nueva `official_releases`**, de solo agregar (trigger, como `economic_events`), con:

| Campo | Tipo | Notas |
|---|---|---|
| `source` | texto | `fed_fomc` o `bls` |
| `series` | enum | `fomc_statement`, `cpi`, `employment_situation` |
| `reference_period` | texto | Mes de referencia (`2013-09`) para el BLS; fecha de la reunión (`2013-01-30`) para el FOMC |
| `status` | enum | `scheduled`, `rescheduled`, `cancelled`, `unscheduled` |
| `scheduled_at` | UTC, terminado en Z | Hora del anuncio. `null` solo si `status = cancelled` |
| `published_at` | UTC, terminado en Z | Desde cuándo se sabía (punto 4). `CHECK (published_at <= scheduled_at)` salvo en las no programadas, donde son iguales |
| `time_rule` | texto | De dónde salió la hora: `release_page` (la página dice la hora) o el nombre de la regla de `config/` |
| `with_press_conference` | booleano o `null` | Solo FOMC. La familia E lo necesita para separar las reuniones con proyecciones |
| `source_url` | texto | La página oficial de la que sale la fila |
| `inserted_at` | UTC | La pone la base |

Clave natural: (`source`, `series`, `reference_period`, `status`, `scheduled_at`). La ingesta es idempotente: una segunda corrida no agrega nada.

**Por qué no la tabla `economic_events`:** su contrato `economic_event.v1` tiene `additionalProperties: false` y no tiene mes de referencia, estado (reprogramado, cancelado) ni la URL de la fuente. Sin el mes de referencia, una fila reprogramada no se puede unir con la original, y una cancelada no se puede representar. Agregar esos campos como obligatorios es un `v2`, que rompe a sus consumidores (el filtro §7.2). Además, el ADR 0010 §6 dejó dicho que el filtro §7.2 no se rellena con otra fuente. Si el dueño quiere que el filtro §7.2 lea también este histórico en el backtest, se hace en otro Issue, con una vista de solo lectura que proyecte `official_releases` a la forma de `economic_event.v1` (`impact = 'high'`, `country = 'US'`), sin copiar filas. Así hay **un solo lugar** para cada dato.

**Quién escribe y quién lee:** lo escribe Python (`intelligence/cfds_intel/calendar`, un submódulo nuevo junto al de Forex Factory, sin tocar su código). Lo leen el estudio de la familia E y, si se aprueba, el motor en Rust. El cálculo de ventanas y retornos alrededor del evento es de Rust (`CLAUDE.md` §1).

### 6. Verificación al cargar

- Cuentas esperadas por año: 8 comunicados programados del FOMC (o los que diga la página del año), 12 CPI y 12 informes de empleo, salvo los cancelados de 2025. Si no coinciden, la carga falla.
- Las fechas del FOMC tienen que coincidir con las del calendario tentativo o su revisión. Si una reunión se movió sin un comunicado que lo documente, se marca y se pregunta.
- Una muestra a mano de 10 filas contra la fuente, anotada en el PR de la ingesta.

## Alternativas consideradas

- **Rellenar `economic_events` con este histórico:** descartada (§5). Rompe el contrato o pierde las reprogramaciones, y mezcla dos usos.
- **Forex Factory histórico:** su feed solo trae la semana en curso. La página del calendario histórico responde 403 a un cliente automático, y leerla con un navegador automatizado está prohibido (ADR 0010 §2).
- **Investing.com, Econoday, Trading Economics u otros calendarios privados:** términos restrictivos o de pago (ADR 0006, "Alternativas"). Además copian las fuentes oficiales: mejor ir al original.
- **Datasets de terceros** (FRED/ALFRED de la Fed de St. Louis, listas en GitHub o en artículos académicos): ALFRED trae **fechas** de vintages (no se verificó si trae la hora), y las listas de terceros no tienen garantía. Sirven, como mucho, para **cruzar** la carga oficial en un test, no como fuente.
- **Robot sobre bls.gov sin datos de contacto:** el BLS lo bloquea (403) y lo prohíbe su política. Descartado.
- **Esperar a que Forex Factory acumule años:** la familia E necesita 2011–2025 ahora; esperar no sirve.
- **Usar la fecha del calendario anual del BLS como `published_at`:** no se pudo documentar cuándo salió cada calendario anual. Se elige el informe anterior, que sí está documentado y es más conservador.

## Riesgos y consecuencias

- **Horas aproximadas antes de 2020.** La regla del FOMC dice "around 12:30 p.m." y "typically ... at or around 2:15 p.m.". Un comunicado pudo salir unos minutos antes o después. Para la familia E, que decide en velas de 15 minutos y ejecuta en la siguiente, alcanza. Para ventanas de minutos, no. El estudio tiene que tratar esas horas como **±15 minutos** y decirlo.
- **Eventos no programados** (las reuniones de emergencia de marzo de 2020): no se sabían antes. Si un estudio los mezcla con los programados para medir la deriva *previa* al anuncio, mira el futuro. El campo `status` los separa, y el estudio los excluye o los analiza aparte.
- **Reprogramaciones y cancelaciones** (cierres de 2013 y 2025): las páginas por año del BLS muestran solo la fecha final. Sin los avisos de reprogramación, una fecha movida parecería conocida desde siempre. Por eso la fila reprogramada lleva su propio `published_at`, o queda fuera.
- **Revisiones:** aquí se guardan fechas y horas, no valores. Las revisiones de los datos del CPI y del empleo no afectan a esta tabla. Las revisiones del **calendario** (por ejemplo el calendario tentativo revisado de 2012) son filas nuevas, nunca se pisan.
- **Solo valen desde su publicación** (`CLAUDE.md` §5b). La regla de §4 lo garantiza fila por fila. Un test de la ingesta comprueba que ninguna fila tenga `published_at` posterior a `scheduled_at` salvo las no programadas (que tienen las dos iguales) y que ningún cálculo use una fila antes de su `published_at`.
- **El sitio cambia:** si la Fed o el BLS cambian las URLs o el formato, la carga falla con error. No se adivina.
- **Muestras chicas:** 8 FOMC y 24 datos del BLS por año dan unos 120 FOMC en 15 años. El programa 2 ya estimó un efecto chico (≈ 0,03 R para la Fed, #466). Este calendario solo habilita **medirlo**, no promete ninguna ganancia.
- **Lo que sigue:** aprobado este ADR, se abre el Issue de la ingesta (contrato, migración, parser, tests y la carga en el Mac mini) y el dueño elige la forma de acceso al BLS (§1.3).
