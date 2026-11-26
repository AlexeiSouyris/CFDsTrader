# 0020 – GDELT 2.0 como feed global de eventos y noticias

- **Estado:** propuesta
- **Fecha:** 2026-11-26
- **Aprobada por:** pendiente. Solo el dueño la aprueba (modo de un solo constructor, `CLAUDE.md`); un agente no la da por aprobada. **Nada de este ADR habilita dinero real ni órdenes reales.**
- **Issue:** #734 (épica #707, eje 4; investigación #711, recomendación 4)
- **Amplía:** [ADR 0006](0006-fuentes-de-noticias.md) (fuentes de noticias oficiales y sin costo). El ADR 0006 sigue vigente: este agrega una fuente, no cambia las que ya hay.
- **Relacionados:** [ADR 0003](0003-modelo-adaptativo-y-contexto.md) §C (el contexto se mide antes de sumar al puntaje), [ADR 0008](0008-modelo-local-para-noticias.md) (modelo local para noticias), [ADR 0012](0012-tesoro-y-vix.md) (ejemplo de regla de hora de publicación), `docs/research/2026-11-terminal-pro-investigacion.md` §3.6, #735 (ADR 0021 de la EIA, hermano)

## Contexto

El dueño pidió una terminal para "leer noticias de todo el mundo" (#707). Hoy el sistema solo lee los RSS oficiales de la Fed y del BCE (ADR 0006). Los medios privados quedaron fuera por sus términos.

La investigación #711 propuso **GDELT** como la mejor base gratuita y legal para "qué pasa en el mundo ahora": eventos codificados, temas, países y tono de las noticias de todo el mundo, con permiso de guardar y redistribuir. La investigación dejó **sin verificar** la cadencia de 15 minutos. Este ADR la verifica y propone cómo usar GDELT sin mirar el futuro.

Como todo el contexto (ADR 0003 §C), GDELT **se registra y se mide, pero no suma al puntaje** hasta validarse con su propio estudio pre-registrado.

### Lo que se comprobó el 26-11-2026

Lectura de las páginas oficiales con WebFetch y `curl` (User-Agent propio de cliente automático, sin datos personales). De los archivos de datos se pidieron solo los **encabezados** HTTP (`HEAD`): no se descargó ningún archivo de datos.

| Punto | Dato | Fuente |
|---|---|---|
| Términos: uso | Los datos están disponibles "for unlimited and unrestricted use for any academic, commercial, or governmental use", sin costo | [About, Terms of Use](https://www.gdeltproject.org/about.html) |
| Términos: redistribución | Se permite "redistribute, rehost, republish, and mirror any of the GDELT datasets", en cualquier forma | Ídem |
| Términos: cita | Todo uso o redistribución "must include a citation to the GDELT Project and a link to this website" (`https://www.gdeltproject.org/`) | Ídem |
| Cadencia | Para GDELT 2.0 (Events y GKG), la página de datos dice "the data updates every 15 minutes" | [Data](https://www.gdeltproject.org/data.html) |
| Inicio de GDELT 2.0 | Los flujos 2.0 empiezan a media mañana del 19-2-2015. Antes solo hay GDELT 1.0, diario | [GDELT 2.0: Our Global World in Realtime](https://blog.gdeltproject.org/gdelt-2-0-our-global-world-in-realtime/) |
| Lista del último lote | `https://data.gdeltproject.org/gdeltv2/lastupdate.txt`: tres líneas (tamaño en bytes, MD5 y URL) para `export` (Events), `mentions` y `gkg`. Hay una lista completa (`masterfilelist.txt`) y versiones aparte para lo traducido (`lastupdate-translation.txt`) | `lastupdate.txt` leído a las 01:13 UTC; blog de GDELT 2.0 |
| Nombre de cada lote | `AAAAMMDDHHMMSS.export.CSV.zip` y `AAAAMMDDHHMMSS.gkg.csv.zip`, en UTC y alineados a 15 minutos | Ídem |
| Hora en Events | `DATEADDED`: cuándo el evento entró a la base, en `YYYYMMDDHHMMSS` y en UTC. Es el campo para trabajar a 15 minutos. Los campos de fecha del evento (`Day`, `SQLDATE`) son la fecha **en que ocurrió**, que puede ser anterior | [Event Codebook V2.0](https://data.gdeltproject.org/documentation/GDELT-Event_Codebook-V2.0.pdf) |
| Hora en GKG | `GKGRECORDID` empieza con la fecha y hora del lote de 15 minutos que lo creó. `V2.1DATE` es la misma para todas las filas de un archivo. Una "T" después del guion marca lo traducido por GDELT | [GKG Codebook V2.1](https://data.gdeltproject.org/documentation/GDELT-Global_Knowledge_Graph_Codebook-V2.1.pdf) |
| Título de la nota | El GKG trae el título en un bloque `<PAGE_TITLE>` del campo de extras **desde el 22-9-2019** (artículos procesados después del mediodía EST de ese día). Antes no hay título | Blog de GDELT, "GKG 2.0 Now Includes Page Titles" (22-9-2019) |
| Datos personales en Events | `Actor1Name` y `Actor2Name` pueden ser **nombres de personas** (el codebook da como ejemplo el nombre formal de un líder político) | Event Codebook V2.0 |
| Datos personales en GKG | `V1PERSONS`, `V2ENHANCEDPERSONS` y `V2.1ALLNAMES` son listas de nombres de personas. `V2.1QUOTATIONS` son citas textuales de personas. El bloque de extras puede traer `<PAGE_AUTHORS>` (autores) desde el 22-4-2016 | GKG Codebook V2.1; blog de GDELT, "New GKG 2.0 Article Metadata Fields" (22-4-2016) |
| Temas | La lista oficial de temas del GKG está en `https://data.gdeltproject.org/api/v2/guides/LOOKUP-GKGTHEMES.TXT` (59 315 líneas). Se verificó que existen todos los temas del §2.1 | Lista leída el 26-11-2026 |
| Códigos CAMEO | Raíz 13 *Threaten*, 15 *Exhibit force posture*, 16 *Reduce relations* (163: *Impose embargo, boycott, or sanctions*), 17 *Coerce*, 18 *Assault*, 19 *Fight*, 20 *Use unconventional mass violence*. `QuadClass` 4 = conflicto material | [CAMEO Manual 1.1b3](https://data.gdeltproject.org/documentation/CAMEO.Manual.1.1b3.pdf), Event Codebook V2.0 |
| `robots.txt` | `data.gdeltproject.org/robots.txt` responde **404** (sin reglas). `www.gdeltproject.org/robots.txt` prohíbe `/data/` en **ese** host (donde están las tablas de códigos), no en `data.gdeltproject.org` | `curl` |
| Límites de pedidos | Ni la página de términos ni el blog de GDELT 2.0 publican límites | Páginas citadas |

**Hora real de publicación de cada lote.** Los encabezados `Last-Modified` de `data.gdeltproject.org` muestran que cada archivo aparece **antes** de la hora de su nombre:

| Archivo | `Last-Modified` (UTC) | Diferencia con la hora del nombre |
|---|---|---|
| `20261126011500.export.CSV.zip` | 26-11-2026 01:04:01 | 11 min antes |
| `20261126010000.export.CSV.zip` | 26-11-2026 00:49:15 | 11 min antes |
| `20261126004500.gkg.csv.zip` | 26-11-2026 00:34:00 | 11 min antes |
| `20230615120000.gkg.csv.zip` | 15-6-2023 11:50:43 | 9 min antes |
| `20200315120000.gkg.csv.zip` | 15-3-2020 11:52:06 | 8 min antes |
| `20150301000000.export.CSV.zip` | 28-2-2015 23:48:41 | 11 min antes |

Dos rarezas, a tener en cuenta:

- `20261126004500.gkg.csv.zip` respondió **404** a las 01:13 UTC y **200** a las 01:22 UTC, aunque su `Last-Modified` dice 00:34. Un archivo puede tardar en poder bajarse más de lo que indica su `Last-Modified`.
- `20261126010000.gkg.csv.zip` seguía en **404** a las 01:22 UTC. Puede haber lotes que faltan, al menos por un rato.

**Tamaño de los archivos** (`Content-Length`, sin descargarlos):

| Tabla | Por lote (15 min) | Ejemplos |
|---|---|---|
| Events (`export`) | 50–150 KB comprimido | 64 336 y 88 390 B (26-11-2026); 53 492 B (15-3-2020); 120 094 B (15-6-2023); 146 170 B (1-3-2015) |
| GKG | 4–9 MB comprimido | 4 039 525 y 4 424 684 B (26-11-2026); 4 268 198 B (15-3-2020); 8 651 692 B (15-6-2023) |

GDELT informó 150 millones de registros del GKG entre el 19-2-2015 y el 31-10-2015 ([blog, 31-10-2015](https://blog.gdeltproject.org/gdelt-2-0-gkg-reaches-150-million-records/)): unos **590 000 por día**, o unos **6 100 por lote**. El volumen de hoy no se publica. Por el tamaño de los archivos, es del mismo orden.

## Decisión (propuesta)

### 1. Fuente y forma de acceso

1. **Solo los archivos crudos de GDELT 2.0** en `https://data.gdeltproject.org/gdeltv2/`: `lastupdate.txt` para saber cuál es el último lote, y `*.export.CSV.zip` y `*.gkg.csv.zip` de cada lote. **No** se usan las API de GDELT (DOC, GEO), ni BigQuery (es de Google y tiene costo), ni copias de terceros.
2. **v1: solo el flujo en inglés.** El flujo traducido (`lastupdate-translation.txt`, unas 65 lenguas) duplica el volumen. Se suma con un cambio en `config/` cuando se haya medido el tamaño del inglés (§5).
3. **No** se lee la tabla `mentions`: v1 no la necesita.
4. **Nunca** se pide la noticia original. Del artículo solo se guardan el título que da GDELT y su enlace (como en el ADR 0006). El texto completo no se guarda ni se muestra.

### 2. Qué se guarda

#### 2.1 GKG: notas de finanzas, bancos centrales y energía

Se guarda una fila por nota del GKG que tenga **al menos un tema** de esta lista. La lista va en `config/gdelt.toml`, no en el código (`CLAUDE.md` §5b). Todos los temas existen en la lista oficial del 26-11-2026:

| Grupo | Temas |
|---|---|
| Bancos centrales y tasas | `ECON_CENTRALBANK`, `ECON_INTEREST_RATES`, `ECON_INFLATION` |
| Mercados | `ECON_STOCKMARKET`, `ECON_CURRENCY_EXCHANGE_RATE`, `ECON_SOVEREIGN_DEBT` |
| Energía y materias primas | `ENV_OIL`, `ECON_OILPRICE`, `ENV_NATURALGAS`, `ECON_NATGASPRICE`, `ECON_GASOLINEPRICE`, `ECON_GOLDPRICE` |
| Geopolítica con efecto económico | `SANCTIONS`, `ECON_TRADE_DISPUTE`, `BLOCKADE`, `CEASEFIRE` |

Los temas muy amplios (`ARMEDCONFLICT`, `MILITARY`, `TERROR`) **no** entran por el GKG: la geopolítica entra por Events (§2.2).

Campos que se guardan de cada nota: el id del registro (`GKGRECORDID`), la hora del lote, el dominio de la fuente (`V2SOURCECOMMONNAME`), el enlace (`V2DOCUMENTIDENTIFIER`), el título (`<PAGE_TITLE>`, vacío antes del 22-9-2019), si fue traducida (la "T" del id), los temas de la lista que tiene, los **códigos de país** de sus lugares (`V2ENHANCEDLOCATIONS`, solo el código de país, que GDELT da en FIPS 10-4 y no en ISO) y el tono de GDELT (`V1.5TONE`: tono, positivo, negativo, polaridad y cantidad de palabras), tal como lo da GDELT.

#### 2.2 Events: geopolítica

Se guarda un evento si es **evento raíz** (`IsRootEvent = 1`), su `EventRootCode` está en {13, 15, 16, 17, 18, 19, 20} y su `NumArticles` llega al mínimo de `config/gdelt.toml`. El mínimo se fija después de medir un día (§5). La lista de códigos también va en `config/`.

Campos que se guardan: `GlobalEventID`, `DATEADDED`, la fecha del evento (`SQLDATE`, solo informativa, ver §3.4), `EventCode`, `EventBaseCode`, `EventRootCode`, `QuadClass`, `GoldsteinScale`, `NumMentions`, `NumSources`, `NumArticles`, `AvgTone`, los códigos CAMEO de país y de tipo de cada actor (`Actor1CountryCode`, `Actor1Type1Code` y los del actor 2), el país del lugar del hecho (`ActionGeo_CountryCode`, en FIPS 10-4) y `SOURCEURL`. Los países de los actores vienen en códigos CAMEO de 3 letras: son otra codificación, y unirlos con los de los lugares necesita una tabla de equivalencias citada.

#### 2.3 Lo que **no** se guarda (datos personales y contenido de terceros)

- **Nombres de personas:** `Actor1Name` y `Actor2Name` de Events, y `V1PERSONS`, `V2ENHANCEDPERSONS` y `V2.1ALLNAMES` del GKG. De los actores solo quedan códigos de país y de tipo (por ejemplo "gobierno"), nunca un nombre.
- **Citas de personas** (`V2.1QUOTATIONS`) y **autores** (`<PAGE_AUTHORS>`).
- **Coordenadas exactas, imágenes y videos** (`V2.1SHARINGIMAGE`, `V2.1RELATEDIMAGES`, `V2.1SOCIALIMAGEEMBEDS`, `V2.1SOCIALVIDEOEMBEDS`) y los campos que no se usan (GCAM, conteos, montos, organizaciones). Si después hace falta alguno que no sea dato personal, se agrega con su Issue.
- **El texto de la nota:** nunca se descarga.
- **Límite que queda:** el título y el enlace pueden nombrar a una persona pública ("X dijo..."). Se guardan como los publicó el medio, pero **no se extraen ni se indexan nombres**, y no habrá búsqueda ni filtro por persona. Si el dueño prefiere no guardar títulos, se muestra solo el dominio y el enlace (§7).

### 3. Hora de cada fila, sin mirar el futuro (`CLAUDE.md` §5b)

1. **`batch_at`:** la hora del nombre del lote (UTC). Es el `DATEADDED` de Events y el prefijo del `GKGRECORDID`. La ingesta comprueba que las tres coincidan. Si no, **ese lote falla** y no se guarda.
2. **`available_at` = la más tardía entre `batch_at` y el `Last-Modified` del archivo.** Es la misma regla para el vivo y para el histórico. Hoy, en todos los ejemplos, da `batch_at`, porque GDELT publica el archivo unos 8–12 minutos antes. Si un archivo se subió tarde (o se volvió a subir), su `Last-Modified` es más tardío, y la fila vale desde ahí.
3. **`retrieved_at`:** cuándo lo bajamos. En vivo, una decisión solo usa filas ya bajadas. Si se bajó antes de `batch_at` (lo normal, ver §4.1), la fila igual vale recién desde `available_at`.
4. **Nunca se usa como hora de disponibilidad** la fecha del evento (`SQLDATE`) ni la de publicación de la nota (`<PAGE_PRECISEPUBTIMESTAMP>`). Las dos son anteriores al momento en que GDELT lo vio, y usarlas sería mirar el futuro. `SQLDATE` se guarda solo como dato.
5. **En el backtest**, una fila se usa en la vela de 15 minutos `t` solo si `available_at + holgura ≤ cierre de t`, y la decisión se ejecuta en `t+1`. La holgura cubre lo que se vio el 26-11-2026: un archivo que recién se pudo bajar entre 28 y 37 minutos después de la hora de su nombre. Va en `config/gdelt.toml`, empieza en **45 minutos** y, después del primer mes en vivo, se fija en el percentil 99 de `retrieved_at − batch_at` medido en `gdelt_batches` (§6), citando esa medición. En vivo no hay holgura: vale lo ya bajado.
6. **Lotes que faltan** (404 o MD5 distinto): quedan registrados como **hueco** en `gdelt_batches`. Se reintentan más tarde (§4). Nunca se rellenan ni se interpolan. Si aparecen después, valen desde su propio `available_at`.
7. **Las filas no se pisan:** las tablas son de solo agregar (trigger, como `economic_events`). Si GDELT vuelve a publicar un lote, sus filas ya guardadas no cambian.

### 4. Descarga respetuosa

1. **Un ciclo cada 15 minutos**, a los :06, :21, :36 y :51. GDELT publica cada lote unos 11 minutos antes de la hora de su nombre, así que a las 01:06 ya está el de 01:15. Cada ciclo hace un pedido a `lastupdate.txt` y, si hay un lote nuevo, baja sus dos archivos (`export` y `gkg`). Son unos **290 pedidos por día**.
2. **Un pedido por vez**, con **al menos 1 s** entre pedidos. Nunca en paralelo.
3. **User-Agent propio de cliente automático, sin datos personales**, como en `config/news.toml`.
4. Se comprueban el **tamaño y el MD5** que da `lastupdate.txt`. Si no coinciden, el lote se marca y no se guarda.
5. **Reintentos:** a lo sumo 3 por archivo, con espera creciente. Después, hueco (§3.6). En cada ciclo se reintentan como mucho 4 lotes atrasados, del más viejo al más nuevo, para no amontonar pedidos tras un corte.
6. Ante **403, 429 o un desafío anti-bots**, el comando se frena con error, avisa (`ops_event`) y **no** reintenta hasta el ciclo siguiente. **Nunca** se esquiva nada: ni proxies, ni rotar IP, ni hacerse pasar por un navegador.
7. **Nada de `www.gdeltproject.org/data/`** por script (su `robots.txt` lo prohíbe). Las tablas de códigos se copian a mano a `config/` cuando hagan falta, citando la fuente.
8. Los `.zip` se procesan y se **borran**: no se guarda el archivo crudo, solo las filas filtradas.

### 5. Tamaño estimado y medición previa

**Descarga** (tráfico, no disco): GKG ≈ 0,4–0,85 GB por día y Events ≈ 5–14 MB por día. Al año, 140–300 GB de tráfico.

**Disco** (filas filtradas en PostgreSQL, con índices). Los porcentajes son **supuestos**, a medir:

| Tabla | Filas por día | Por fila | Por día | Por año |
|---|---|---|---|---|
| `gdelt_articles` | 30 000–90 000 (5–15 % de ~590 000 notas) | ~600 B | 18–54 MB | 6,5–20 GB |
| `gdelt_events` | 10 000–40 000 (10–20 % de 100 000–200 000 eventos, supuesto) | ~300 B | 3–12 MB | 1–4,5 GB |
| `gdelt_batches` | 192 | ~200 B | < 0,1 MB | < 15 MB |
| **Total** | | | **~21–66 MB** | **~8–25 GB** |

El Mac mini tenía **111 GiB libres** el 26-11-2026 (`df`). Por eso:

1. **Antes de la carga continua**, la ingesta corre **un día completo** de prueba (96 lotes) y anota en su PR las filas y los MB reales por tabla. Si el total anual estimado pasa de **10 GB**, se ajustan los filtros (temas, `NumArticles`) antes de seguir, con el número a la vista.
2. **Sin carga histórica larga en v1.** Solo los últimos 7 días, para medir. Una carga desde 2019 (con títulos) serían 55–175 GB, más de lo que conviene en el Mac mini. Si el dueño la quiere, se decide aparte, con el tamaño medido.
3. Un **tope de tamaño** en `config/gdelt.toml` (por ejemplo 20 GB para las tablas de GDELT) dispara un aviso (`ops_event`). No borra nada solo.

### 6. Contratos, tablas y módulo (a definir en el Issue de la ingesta)

Contratos primero (`CLAUDE.md` §1): se definen en el PR de la ingesta o en uno anterior, no en este.

| Pieza | Nombre propuesto | Qué es |
|---|---|---|
| Contrato | `contracts/gdelt_article.v1.schema.json` | Una nota del GKG filtrada (§2.1), con `batch_at`, `available_at` y `retrieved_at` en UTC terminados en `Z` |
| Contrato | `contracts/gdelt_event.v1.schema.json` | Un evento filtrado (§2.2), sin nombres de actores |
| Contrato | `contracts/gdelt_batch.v1.schema.json` | Un lote leído o faltante: hora, tipo de archivo, tamaño, MD5, `Last-Modified`, `retrieved_at`, estado (`ok`, `missing`, `md5_mismatch`), filas leídas y guardadas |
| Tabla | `gdelt_articles` | Solo agregar. Clave: `gkg_record_id` |
| Tabla | `gdelt_events` | Solo agregar. Clave: `global_event_id` |
| Tabla | `gdelt_batches` | Solo agregar. Clave: (`batch_at`, `file_kind`, `retrieved_at`). Sirve de registro de huecos y de las horas reales |
| Config | `config/gdelt.toml` | URL base, User-Agent, cadencia, temas, códigos CAMEO, `NumArticles` mínimo, holgura del backtest, flujo traducido (apagado), tope de tamaño |
| Módulo | `intelligence/cfds_intel/gdelt` (Python, nuevo) | Ingesta. Se agrega al Registro de módulos en el PR de la ingesta |
| Migraciones | Las próximas libres al momento de la ingesta | Coordinar el número con los PRs abiertos |

**Por qué no `news_item.v1` ni la tabla `news_items`:** esas noticias pasan por el intérprete local (ADR 0008), que no da abasto con decenas de miles de notas por día, y su contrato no tiene tono, temas ni países. Mezclar las dos cosas confundiría "noticias oficiales interpretadas" con "contexto global de GDELT".

**Quién escribe y quién lee:** escribe Python (`cfds_intel.gdelt`). Cualquier agregado (por ejemplo conteos o tono medio por tema y país cada 15 minutos) se define en otro Issue, con su spec, en el lenguaje que corresponda según `CLAUDE.md` §1. Mostrarlo en la terminal necesita su propio contrato del gateway, en otro Issue.

### 7. Cita y uso

1. **Toda vista o exportación** que muestre datos de GDELT dice "Fuente: The GDELT Project" con el enlace `https://www.gdeltproject.org/`, como piden los términos, más el enlace a la nota original.
2. El tono es **el de GDELT**, calculado por máquina, y se muestra así ("tono según GDELT"). No es una interpretación nuestra.
3. Es **contexto**: no suma al puntaje hasta que un estudio pre-registrado lo valide (ADR 0003 §C). Ningún texto lo presenta como señal ni promete ganancias.
4. **Solo en el Mac mini:** la ingesta se niega a guardar si no está `CFDS_DATA_MACHINE` (como las cargas de IG). En la nube, solo tests con filas inventadas.

## Alternativas consideradas

- **API DOC 2.0 de GDELT** (búsqueda de notas con título): es cómoda, pero es una API de búsqueda, con límites que no se verificaron y sin una hora de lote clara. Los archivos crudos tienen hora de lote, MD5 y la regla de §3. Descartada para v1.
- **Google BigQuery** (GDELT completo, actualizado cada 15 minutos): tiene costo y cuenta de Google. Va contra pagar solo cuando hace falta.
- **GDELT 1.0** (diario, desde 1979): no sirve para 15 minutos. Podría servir para estudios diarios largos, con otro Issue.
- **Medios por RSS o scraping** (Reuters, FT, Bloomberg): sus términos no permiten guardar (ADR 0006). Descartado.
- **Pasar las notas de GDELT por el intérprete local (ADR 0008):** son decenas de miles por día. Fuera de v1. Se podría con una muestra chica, en otro Issue.
- **Guardar todo el GKG sin filtrar:** cientos de GB por año. Descartado.

## Riesgos y consecuencias

- **Ruido de la codificación automática:** GDELT codifica por máquina. Hay eventos falsos o repetidos (la misma noticia en muchos medios), países mal asignados y errores de traducción. El codebook del GKG documenta un error de códigos de país del 19-2-2015 al 1-3-2015. Por eso es contexto que se mide, no una señal.
- **Huecos y atrasos:** se vio un lote que faltaba y otro que tardó en poder bajarse. Se registran (`gdelt_batches`) y nunca se rellenan. Si un estudio cae en un hueco, lo dice.
- **El volumen puede cambiar:** si GDELT procesa más notas, crece el disco. La medición de §5 y el tope de tamaño lo vigilan.
- **Títulos solo desde el 22-9-2019:** antes de esa fecha, una nota se muestra solo con su dominio y su enlace.
- **Términos que pueden cambiar:** se citaron el 26-11-2026. Si GDELT los cambia, se revisa este ADR.
- **Hora del histórico:** el `Last-Modified` de un archivo viejo dice cuándo se subió. Si GDELT lo volvió a subir, la fila vale desde más tarde (más conservador, nunca antes). Si alguna vez el `Last-Modified` fuera anterior a cuando el archivo se pudo bajar, la holgura de §3.5 lo cubre.
- **Contenido de terceros:** títulos y enlaces son de los medios. Se muestran con su enlace, sin el texto. GDELT permite redistribuir sus datos, pero el artículo es del medio.

## Pendiente del dueño al aprobar

1. **Aprobar o no** GDELT como fuente (este ADR).
2. **Títulos:** guardarlos (recomendado, §2.3) o mostrar solo el dominio y el enlace.
3. **Flujo traducido:** apagado en v1 (recomendado) o prendido desde el principio.
4. **Retención:** guardar todo (recomendado hasta medir) o un período fijo (por ejemplo 24 meses) para las notas, una vez medido el tamaño.
5. **Carga histórica:** solo 7 días (recomendado) o más, con el tamaño medido a la vista.

Aprobado este ADR, se abre el Issue de la ingesta: contratos, migraciones, `config/gdelt.toml`, el módulo, tests (huecos, MD5 distinto, `Last-Modified` tardío, `DATEADDED` distinto de la hora del lote, filas sin título, campos de personas descartados) y el día de medición en el Mac mini.
