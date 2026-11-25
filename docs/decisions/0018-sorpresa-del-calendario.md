# 0018 – Guardar el pronóstico, el dato anterior y el dato real del calendario (sorpresa)

- **Estado:** aceptada
- **Fecha:** 2026-11-25
- **Aprobada por:** el dueño (Alexei), el 25-11-2026, en el chat con su agente, textual: «Si lei los terminos y si podemos guardar el forecast, asique hacelo.» (modo de un solo constructor, `CLAUDE.md`). Registrado en [#667](https://github.com/AlexeiSouyris/CFDsTrader/issues/667#issuecomment-7170508946). Con eso queda cumplida la condición previa del §1.1: el dueño leyó los términos de Forex Factory y permiten guardar `forecast`. No eligió formato, así que vale la opción recomendada del §3: dos contratos y tablas nuevas (`calendar_estimate.v1` y `release_actual.v1`), sin tocar `economic_event.v1`. Este ADR no habilita dinero real.
- **Issue:** #667 (plan del autoanálisis, #658, ítem 8, **N2**)
- **Modifica:** [ADR 0010](0010-calendario-forex-factory.md) §7 ("Qué se guarda"). El resto del ADR 0010 sigue vigente: misma URL, un pedido por hora como máximo, sin scraping ni esquivar bloqueos.
- **Relacionados:** [ADR 0006](0006-fuentes-de-noticias.md) §3 (regla de publicación), [ADR 0016](0016-calendario-historico-oficial.md) (calendario oficial de FOMC, CPI y empleo; política del BLS), `docs/analysis/2026-11-autoanalisis.md` §6

## Contexto

Lo que mueve el precio en un dato macro no es el dato, sino **la sorpresa**: el dato real menos lo que se esperaba (Balduzzi, Elton y Green 2001; Andersen, Bollerslev, Diebold y Vega 2003). Hoy no la podemos medir nunca:

- El feed de Forex Factory que ya leemos cada hora (ADR 0010) trae `forecast` y `previous` de cada evento, pero el ADR 0010 §7 decidió **no guardarlos** porque el contrato `economic_event.v1` no los tiene y el filtro §7.2 no los usa.
- El feed **no trae el dato real** (`actual`): solo `title`, `country`, `date`, `impact`, `forecast` y `previous` (ADR 0010, tabla de lo comprobado).
- El ADR 0016 guarda **cuándo** salió cada CPI y cada informe de empleo, no **qué** dijo, y ya avisa que la sorpresa "necesita otra fuente y otro ADR". Es este.

El feed no tiene historia (ADR 0010 §6). Cada semana que pasa sin guardar `forecast` es una semana de sorpresas perdida para siempre. Por eso conviene decidir ya, aunque el primer estudio que la use esté lejos.

Lo que se comprobó el 25-11-2026 (WebFetch; sin pedir el feed, que ya lo lee el cron):

### Forex Factory: términos

| Punto | Dato |
|---|---|
| `https://www.forexfactory.com/notices` (términos) | **403** a un cliente automático, otra vez (ya pasó el 23-11, ADR 0010). |
| `https://www.faireconomy.media` y `https://nfs.faireconomy.media/` | Redirigen (301) a `forexfactory.com`, que responde 403. No hay términos propios del feed a la vista. |
| Lo único documentado del feed | El aviso del 429: el archivo "is only updated once per hour" y pedirlo de más "can result in being blocked" (ADR 0010). Nada sobre guardar sus valores. |

**Los términos seguían sin leerse** al escribir la propuesta (el dueño los leyó después; ver «Aprobada por»). El ADR 0010 ya dejó pendiente que el dueño los lea en el navegador. Este ADR depende de lo mismo, con una pregunta más concreta (Decisión §1.1).

### Fuentes oficiales del dato real

| Fuente | Lo que dice | Fuente del dato |
|---|---|---|
| BLS, términos de la API | Uso secundario: los datos "do not, and should not, include controls over its end use". Hay que citar la fecha en que se bajaron y decir que el BLS "cannot vouch for the data or analyses derived". Pueden bloquear a quien supere los límites | [termsOfService](https://www.bls.gov/developers/termsOfService.htm) |
| BLS, límites | v1 (sin registro): 25 consultas por día, 25 series por consulta, 10 años. v2 (con registro): 500 por día. Las dos: 50 pedidos cada 10 s como máximo. v1 "does not require registration and is open for public use" | [api_faqs](https://www.bls.gov/developers/api_faqs.htm), [api_features](https://www.bls.gov/bls/api_features.htm) |
| BLS, demora | Hay "one day lag between published data and its availability" en la API | [api_features](https://www.bls.gov/bls/api_features.htm) |
| BLS, ¿primer dato o revisado? | **No lo dice.** Hay que suponer que la API da el **valor vigente** (revisado), no el primero publicado | Ídem |
| BLS, sitio web | Dominio público, citar la fuente. Robot sin datos de contacto: 403 y prohibido (ADR 0016) | [Copyright](https://www.bls.gov/opub/copyright-information.htm) |
| FRED/ALFRED (Fed de St. Louis) | Pide clave. Las series de terceros solo para "your own personal use" sin pedir permiso al dueño del dato. ALFRED da cada versión (vintage) por **fecha**, sin hora (`YYYY-MM-DD`) | [FRED terms](https://fred.stlouisfed.org/docs/api/terms_of_use.html), [realtime_period](https://fred.stlouisfed.org/docs/api/fred/realtime_period.html) |
| BEA (PIB, PCE) | Registro con nombre y correo. Los términos (PDF) **no se pudieron leer** | [signup](https://apps.bea.gov/api/signup/) |
| Eurostat y BCE | Las páginas de reutilización **no se pudieron leer** (404 en las URLs probadas) | — |

## Decisión

### 1. Forex Factory: guardar `forecast` y `previous`

1. **Condición previa:** el dueño lee los términos de Forex Factory en el navegador y confirma en #667 que **no prohíben guardar** los valores del Calendar Export para uso personal. Si los prohíben, o no dicen nada claro, el punto 1 no se implementa y queda solo el punto 2 (fuentes oficiales).
2. Se guarda lo que ya llega en cada lectura del feed, **sin pedidos extra**: el `forecast` y el `previous` de cada evento, **como texto tal cual lo publica Forex Factory** (por ejemplo `"0.3%"`, `"215K"`, `"-1.2B"`, o vacío). Además, si se puede interpretar sin ambigüedad, el número en `Decimal` y su unidad (`%`, `K`, `M`, `B`, `T`). Si no se puede, el número queda en `null`: **no se adivina**.
3. **Instantáneas, de solo agregar.** El `forecast` cambia durante la semana. Cada vez que el par (`forecast`, `previous`) de un evento **cambia**, es una fila nueva con `seen_at` = la primera lectura en que apareció ese valor. Nada se pisa ni se borra. Una lectura sin cambios no agrega nada.
4. **Anti look-ahead:** el consenso que vale para un evento es **la última instantánea con `seen_at` anterior a `scheduled_at`**. Una instantánea vista después de la hora del evento se guarda (sirve para ver revisiones del `previous`) pero no cuenta como consenso.
5. **Qué es ese `forecast`:** no se verificó cómo lo arma Forex Factory (encuesta, promedio de analistas o estimación propia). Se trata como "el pronóstico que se veía en Forex Factory", **no** como el consenso de una encuesta. Un estudio que lo use lo dice.

### 2. El dato real: solo de fuentes oficiales

1. **Primera serie: CPI y empleo de EE. UU. (BLS)**, las mismas del ADR 0016, con su `published_at` y su hora de embargo ya guardados en `official_releases`.
2. **Vía:** la **API pública del BLS, v1, sin registro** ("open for public use"), no el sitio web. Una consulta por publicación, **el día hábil siguiente** al dato (por la demora de un día de la API), con todas las series de esa publicación en la misma consulta. Son unas 2 consultas por mes: muy lejos del límite de 25 por día. User-Agent propio sin datos personales (como `config/news.toml`). Ante un 403 o un 429, frena sin reintentar ni esquivar (ADR 0016 §1.4).
3. **Primer dato, no el revisado:** como la API da el valor vigente, el valor guardado es el que había **en esa lectura**, con `retrieved_at`. Se guarda como "primer dato" solo si se leyó **antes de la publicación siguiente de la misma serie** (antes de que el BLS lo revise). Si no, se guarda marcado como `revised` y no entra en la sorpresa.
4. **Series concretas** (IDs del BLS, por ejemplo la variación del CPI total y subyacente, el empleo no agrícola y la tasa de desempleo) y la correspondencia con el `title` de Forex Factory van en `config/`, con el ID oficial y su fuente. Nada en el código.
5. **Hora del dato real:** `released_at` = la hora de embargo oficial de `official_releases` (por ejemplo 08:30 ET), no la hora de la lectura. `retrieved_at` es la hora en que lo leímos. Un estudio que simule operar en tiempo real usa el dato desde `released_at`; uno que lo use para decidir algo en vivo, desde `retrieved_at`.
6. **Otras fuentes** (BEA para PIB y PCE, Eurostat, BCE, otros bancos centrales): **fuera de este ADR** hasta leer sus términos. Cada una se agrega con su Issue y, si sus términos piden algo nuevo (clave, registro, datos personales), con su ADR.
7. **Sin dato real oficial** para un evento, la sorpresa de ese evento no existe. **No se toma el `actual` de Forex Factory ni de otro agregador** (no viene en el feed, y leer la página está prohibido por el ADR 0010 §2).
8. Se cita la fuente ("U.S. Bureau of Labor Statistics") con la fecha de la lectura, como piden sus términos.

### 3. Cómo se guarda

**No se toca `economic_event.v1` ni la tabla `economic_events`.** El filtro §7.2 sigue igual. Los datos nuevos van en dos contratos y dos tablas nuevas, de solo agregar (trigger, como `economic_events` y `official_releases`):

**`contracts/calendar_estimate.v1`, tabla `calendar_estimates`** (Python escribe, desde la ingesta de Forex Factory):

| Campo | Tipo | Notas |
|---|---|---|
| `source`, `event`, `country`, `scheduled_at` | como `economic_event.v1` | Identifican el evento de `economic_events` al que pertenece |
| `forecast_raw`, `previous_raw` | texto o `null` | Tal como vienen en el feed |
| `forecast_value`, `previous_value` | `numeric` o `null` | Solo si se interpretan sin ambigüedad |
| `unit` | enum o `null` | `percent`, `thousands`, `millions`, `billions`, `trillions`, `level` |
| `seen_at` | UTC, terminado en Z | Primera lectura con este valor |
| `inserted_at` | UTC | La pone la base |

Clave natural: (`source`, `event`, `country`, `scheduled_at`, `forecast_raw`, `previous_raw`).

**`contracts/release_actual.v1`, tabla `release_actuals`** (Python escribe, desde la API del BLS):

| Campo | Tipo | Notas |
|---|---|---|
| `source` | texto | `bls` |
| `series_id` | texto | ID oficial de la serie |
| `reference_period` | texto | `2026-10`, igual que en `official_releases` |
| `value` | `numeric` | Decimal exacto, nunca `float` |
| `unit` | enum | Como arriba |
| `vintage` | enum | `first` o `revised` (Decisión §2.3) |
| `released_at` | UTC | La hora de embargo, unida a `official_releases` |
| `retrieved_at` | UTC | Cuándo lo leímos |
| `source_url` | texto | El endpoint oficial consultado |
| `inserted_at` | UTC | La pone la base |

Clave natural: (`source`, `series_id`, `reference_period`, `vintage`, `value`).

**La sorpresa no se guarda:** es un cálculo. Lo hace **Rust** (`CLAUDE.md` §1), al leer las dos tablas, con la regla del §1.4 (último `forecast` antes de `scheduled_at`) y la correspondencia de `config/`. Así hay un solo lugar para cada dato y un solo lugar para el cálculo.

### 4. Lo que no cambia

- El ADR 0010 §2 a §6: misma URL, como máximo un pedido por hora, sin scraping, sin esquivar Cloudflare, sin histórico.
- No se rellena el pasado. La sorpresa existe **desde el día en que empiece la ingesta**.
- Ningún uso en señales ni en el backtest sin otro Issue y sin muestra suficiente (ver Consecuencias).

## Alternativas consideradas

- **`economic_event.v2` con `forecast`, `previous` y `actual`** (lo que proponía el autoanálisis): descartada como primera opción. `v1` tiene `additionalProperties: false`, así que un `v2` obliga a migrar al filtro §7.2, que no los usa. Además, el `forecast` cambia en la semana, y meterlo en la clave de `economic_events` llenaría el filtro de eventos repetidos; dejarlo fuera de la clave obligaría a pisar filas, y la tabla es de solo agregar. Dos tablas aparte no rompen nada. **Si el dueño prefiere un solo contrato, se puede hacer el `v2`**: lo elige al aprobar.
- **Tomar el `actual` de Forex Factory:** no viene en el feed, y leer la página está prohibido (ADR 0010 §2).
- **ALFRED/FRED para el primer dato:** tiene las versiones viejas, y con eso hasta permitiría rellenar el pasado. Pero pide clave, las fechas no tienen hora, y las series de terceros solo son para uso personal sin permiso. Queda como **verificación cruzada** en un test o como fuente de otro ADR, si el dueño quiere estudiar el pasado.
- **Robot sobre el sitio del BLS:** su política exige datos de contacto del dueño en el robot (ADR 0016). La API es la vía documentada para leer valores.
- **API v2 del BLS (con registro):** más cupo, pero pide registrar al dueño y renovar cada año. Con 2 consultas por mes, v1 alcanza.
- **No hacer nada:** cada semana sin guardar es dato perdido para siempre (el feed no tiene historia).

## Consecuencias

- **Cumplido (25-11-2026):** el dueño leyó los términos de Forex Factory (§1.1) y aprobó este ADR (ver «Aprobada por»).
- **Lo que sigue,** cada uno con su Issue y su PR: (1) los contratos `calendar_estimate.v1` y `release_actual.v1` con sus migraciones; (2) guardar las instantáneas en la ingesta de Forex Factory (sin pedidos extra); (3) la lectura de la API del BLS, en el cron del Mac mini, el día hábil siguiente a cada publicación; (4) la correspondencia de series en `config/`.
- Hay que permitir `api.bls.gov` en la red del entorno.
- **Muestras chicas por mucho tiempo:** unas 2 sorpresas del BLS por mes. Con un año hay unas 24: no alcanza para concluir nada sobre el precio. Este ADR solo empieza a **juntar** el dato; no promete ninguna ganancia.
- **Riesgos:** el feed de Forex Factory puede cambiar de formato o de condiciones sin aviso (ya dicho en el ADR 0010); la API del BLS puede cambiar o bloquear; un `forecast` con formato raro queda solo como texto. En todos los casos la ingesta falla con un mensaje claro y no inventa valores.
