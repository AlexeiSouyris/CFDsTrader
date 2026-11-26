# 0021 – Inventarios y producción semanales de energía de la EIA (EE. UU.)

- **Estado:** propuesta
- **Fecha:** 2026-11-26
- **Aprobada por:** pendiente. Solo el dueño la aprueba (modo de un solo constructor, `CLAUDE.md`); un agente no la da por aprobada. **Nada de este ADR habilita dinero real ni órdenes reales.**
- **Issue:** #735 (épica #707, eje 4; investigación #711, recomendación 5)
- **Relacionados:** [ADR 0003](0003-modelo-adaptativo-y-contexto.md) §C (el contexto se mide antes de sumar al puntaje), [ADR 0006](0006-fuentes-de-noticias.md) §3 (un evento existe desde que se publicó su calendario), [ADR 0012](0012-tesoro-y-vix.md) (fuente oficial con hora de publicación), [ADR 0013](0013-materias-primas-oro.md) (materias primas), [ADR 0016](0016-calendario-historico-oficial.md) (calendario oficial e histórico), [ADR 0018](0018-sorpresa-del-calendario.md) (primer dato y dato revisado), `docs/research/2026-11-terminal-pro-investigacion.md` §3.2, #734 (ADR 0020 de GDELT, hermano)

## Contexto

La épica #707 pide un panel de "mundo en tiempo real" con materias primas. La investigación #711 propuso la **EIA** (U.S. Energy Information Administration) como fuente oficial y gratuita de los datos semanales de energía que mueven al petróleo y al gas: inventarios de crudo y gasolina, producción de crudo y almacenamiento de gas natural.

Son series **semanales** que se publican a una **hora fija**, con feriados, atrasos y algunas revisiones. Si se usa el dato antes de su publicación, el backtest mira el futuro (`CLAUDE.md` §5b). Este ADR fija desde cuándo vale cada dato.

Como todo el contexto (ADR 0003 §C), estos datos **se registran y se miden, pero no suman al puntaje** hasta validarse con su propio estudio pre-registrado.

### Lo que se comprobó el 26-11-2026

Lectura de las páginas oficiales con `curl` (User-Agent propio de cliente automático, sin datos personales) y WebFetch. **No se llamó a la API**: pide una clave, y la clave la crea el dueño.

#### Términos

| Punto | Dato | Fuente |
|---|---|---|
| Dominio público | "U.S. government publications are in the public domain and are not subject to copyright protection. You may use and/or distribute any of our data, files, databases, reports, graphs, charts, and other information products that are on our website..." | [Copyrights and Reuse](https://www.eia.gov/about/copyrights_reuse.php) |
| Cita | "you should use an acknowledgment, which includes the publication date, such as: \"Source: U.S. Energy Information Administration (Oct 2008).\"" | Ídem |
| Logo | El logo de la EIA es marca registrada y no se usa sin permiso | Ídem |
| Términos de la API | "Access to or use of the API or its content constitutes acceptance to this Agreement." Uso permitido: "search, display, analyze, retrieve, view and otherwise \"get\" information from EIA data" | [API Terms of Service](https://www.eia.gov/opendata/terms-of-service.php) |
| No alterar | "You may not modify or falsely represent content accessed through the API and still claim the source is the EIA." Tampoco se puede usar el nombre de la EIA para sugerir que respalda un producto | Ídem |
| Límites | "If the EIA reasonably believes that you have attempted to exceed or circumvent these limits, your ability to use the API may be temporarily or permanently blocked." | Ídem |

#### API

| Punto | Dato | Fuente |
|---|---|---|
| Clave | "To call our API, you must use the unique API key assigned to you." Es gratis: se registra un email en la web y la clave llega a ese email. Va en la URL como parámetro `api_key=` | [API Documentation](https://www.eia.gov/opendata/documentation.php), [Open Data](https://www.eia.gov/opendata/) |
| Ritmo | Los programas automáticos "must throttle the number of the requests they make per second and per hour". Si se pasan, la clave se suspende un rato y vuelve sola. **No publica los números** del límite | API Documentation |
| Tamaño de respuesta | Hasta 5 000 filas por respuesta en JSON. Más, con paginación (`offset`, `length`) | Ídem |
| Ids de serie | La API v2 acepta los ids de la API v1 con la ruta `/v2/seriesid/<id>` (ejemplo de la documentación: `ELEC.SALES.CO-RES.A`) | Ídem |
| Archivos masivos | Sin clave. "The API Bulk Download Update runs twice daily at 5 a.m. and 3 p.m. eastern time." | Open Data |
| `robots.txt` | `www.eia.gov/robots.txt` prohíbe, entre otras, `/archive/` y `/*archive/` (es decir, los archivos de informes viejos). `api.eia.gov/robots.txt` responde 403 por falta de clave. `ir.eia.gov` (el servidor de publicación) responde "Access Restricted" fuera de la ventana de publicación | `curl` |

#### Informe semanal del petróleo (Weekly Petroleum Status Report, WPSR)

| Punto | Dato | Fuente |
|---|---|---|
| Hora normal | "The standard release time and day of the week will be at 10:30 a.m. eastern time on Wednesday with the following exceptions. All times are eastern." | [WPSR Schedule](https://www.eia.gov/petroleum/supply/weekly/schedule.php) |
| Excepciones 2026–2028 | Semanas con feriado: **jueves 12:00 p.m.** (por ejemplo, datos al 6-11-2026 publicados el jueves 12-11-2026 por el Veterans Day). La página solo muestra las excepciones vigentes y futuras | Ídem |
| Qué sale a qué hora | Las tablas en CSV y XLS a las 10:30; el informe completo en PDF a la 1:00 p.m. Período: la semana que termina el viernes anterior. "For weeks which include holidays (or have other disruptions to normal operations), releases are delayed by one day." | [Appendix B, Explanatory Notes](https://www.eia.gov/petroleum/supply/weekly/pdf/appendixb.pdf), [WPSR](https://www.eia.gov/petroleum/supply/weekly/) |
| Revisiones | "EIA will disseminate revised weekly data only if the revision is expected to substantively affect understanding of U.S. petroleum supplies. [...] If a revision is necessary, it will be disseminated in the next regularly scheduled release of the weekly products." Hay una hoja de erratas (`WPSR_ErrataSheet.xlsx`) | Appendix B, p. 40; WPSR |
| Producción de crudo | Una vez por mes, la semana del Short-Term Energy Outlook, la EIA puede "re-benchmark the weekly production estimate". Es un ajuste del dato de esa semana, avisado en la página | WPSR, "Re-benchmarking Notice" |
| Historial de publicaciones | El índice de informes viejos lista cada publicación con su **fecha de publicación** y la fecha de fin de los datos, desde **agosto de 2011**. No da la hora. Hay atrasos: por ejemplo, datos al 3-11-2023 publicados el 15-11-2023 | [Índice de informes viejos](https://www.eia.gov/petroleum/supply/weekly/archive/) (leído una vez con WebFetch; el `robots.txt` lo excluye para robots) |
| Cambio de formato | Desde el 10-6-2026 la EIA cambia cómo **difunde** los archivos del WPSR (nuevos CSV y JSON), sin cambiar los datos | [Aviso del WPSR](https://www.eia.gov/petroleum/supply/weekly/wpsr_notice_06102026.php) |

#### Informe semanal de almacenamiento de gas natural (Weekly Natural Gas Storage Report, WNGSR)

| Punto | Dato | Fuente |
|---|---|---|
| Hora normal | "The standard release time and day of the week will be at 10:30 a.m. eastern time on Thursdays with the following exceptions." | [WNGSR Schedule](https://ir.eia.gov/ngs/schedule.html) |
| Excepciones | Feriados, a veces con poco aviso: "January 8, 2025 - (Updated) Wednesday 12:00 p.m. National Day of Mourning"; "December 29, 2025 - (Updated) Monday 12:00 p.m. Christmas Day" | Ídem |
| Última publicación | "for week ending November 20, 2026 \| Released: November 25, 2026 at 12:00 p.m. \| Next Release: December 3, 2026" (Thanksgiving) | [WNGSR](https://ir.eia.gov/ngs/ngs.html) |
| Revisiones | Se publican en el informe regular si suman al menos **4 Bcf**. Si llegan a **10 Bcf**, hay una publicación **fuera de calendario** a las **2:00 p.m.** ET de un día hábil, avisada entre la 1:00 y la 1:10 p.m. Las reclasificaciones solo salen en el informe regular | [Revision Policy](https://ir.eia.gov/ngs/revisions.html) (actualizada el 26-1-2017) |
| Base histórica | "The historical database is updated weekly and contains the most current set of estimates available." Desde noviembre de 2015 hay un archivo con el valor original de cada semana revisada | Ídem |
| Historial de publicaciones | **No se encontró** una lista oficial de las fechas de publicación pasadas del WNGSR | Búsqueda del 26-11-2026 |

#### Series (títulos leídos en las páginas públicas de cada serie)

| Id (API v1) | Serie | Informe | Unidad |
|---|---|---|---|
| `PET.WCESTUS1.W` | Weekly U.S. Ending Stocks excluding SPR of Crude Oil | WPSR | miles de barriles |
| `PET.W_EPC0_SAX_YCUOK_MBBL.W` | Weekly Cushing, OK Ending Stocks excluding SPR of Crude Oil | WPSR | miles de barriles |
| `PET.WCSSTUS1.W` | Weekly U.S. Ending Stocks of Crude Oil in SPR | WPSR | miles de barriles |
| `PET.WGTSTUS1.W` | Weekly U.S. Ending Stocks of Total Gasoline | WPSR | miles de barriles |
| `PET.WDISTUS1.W` | Weekly U.S. Ending Stocks of Distillate Fuel Oil | WPSR | miles de barriles |
| `PET.WCRFPUS2.W` | Weekly U.S. Field Production of Crude Oil | WPSR | miles de barriles por día |
| `PET.WPULEUS3.W` | Weekly U.S. Percent Utilization of Refinery Operable Capacity | WPSR | porcentaje |
| `NG.NW2_EPG0_SWO_R48_BCF.W` | Lower 48 States Natural Gas Working Underground Storage | WNGSR | miles de millones de pies cúbicos (Bcf) |

Los títulos se leyeron en `www.eia.gov/dnav/...`. El id exacto en la API (con el prefijo `PET.` o `NG.` y el sufijo `.W`) **se comprueba con la clave** en el Issue de la ingesta. Si alguno no existe, la ingesta falla.

**Producción de gas natural:** no se verificó una serie **semanal oficial** de la EIA. La producción de gas sale por mes. Queda fuera de v1.

## Decisión (propuesta)

### 1. Fuente y forma de acceso

1. **La API v2 oficial** (`https://api.eia.gov/v2/`), con los ids de la tabla por la ruta `/v2/seriesid/`. La misma vía para el histórico y para el dato en vivo: **un solo camino** para cada dato.
2. **Nada de scraping** de páginas HTML para los valores. Las páginas solo se leen para el **calendario** (§3).
3. **Nada de terceros** que copien la EIA (por ejemplo FRED, que además prohíbe guardar sus datos).
4. Las series van en `config/eia.toml`, no en el código. Agregar una serie es un cambio de config con su Issue.

### 2. La clave de la API

1. **La crea el dueño**, registrando su email en `https://www.eia.gov/opendata/register.php`. Registrarse y usar la API **es aceptar sus términos** (ver arriba): eso lo decide el dueño. **Ningún agente se registra, acepta términos ni usa el email del dueño.**
2. **Vive solo en el `.env` del Mac mini**, como `EIA_API_KEY`. **Nunca** en el repo, ni en tests, ni en `config/`, ni en Issues o PRs.
3. **`.env.example`** recibe en el PR de la ingesta la variable con un **valor falso** y un comentario (por ejemplo `EIA_API_KEY=fake-eia-key-0000`), como las demás claves.
4. **Nunca en los logs.** La clave viaja en la URL (`api_key=`), así que el cliente:
   - nunca imprime la URL completa;
   - borra `api_key` de todo mensaje de error, excepción y log (`[REDACTED]`), con un test que lo compruebe;
   - guarda el `source_url` **sin** la clave.
5. **Sin clave, el comando falla** con un error claro. No hay plan B silencioso.
6. **Si la clave se filtra**, el dueño pide una nueva en la web de la EIA y cambia el `.env`. El agente que lo note avisa y no la repite.

### 3. Hora de publicación, sin mirar el futuro (`CLAUDE.md` §5b)

Cada dato tiene dos horas, como en el ADR 0018 §2.5:

- **`released_at`:** la hora oficial de publicación del informe que trajo ese dato. Un estudio que simula operar en tiempo real usa el dato **desde `released_at`**.
- **`retrieved_at`:** cuándo lo leímos. Una decisión **en vivo** usa el dato desde `retrieved_at`. Nunca es anterior a `released_at` (`CHECK` en la tabla).

En el backtest, un dato se usa en la vela `t` solo si `released_at ≤ cierre de t`, y la decisión se ejecuta en `t+1`.

#### 3.1 Calendario de publicaciones

1. **El calendario se guarda** en su propia tabla (§5): un registro por publicación de cada informe, con la semana que cubre, la hora y desde cuándo se sabía.
2. **Desde ahora:** la ingesta lee **una vez por semana** las páginas de calendario del WPSR y del WNGSR. Cada lectura queda guardada con su hora. Un cambio (un feriado nuevo, un atraso) es una **fila nueva**, nunca se pisa (ADR 0006 §3).
3. **Hora normal:** WPSR los **miércoles 10:30 ET** (semana que termina el viernes anterior); WNGSR los **jueves 10:30 ET** (semana que termina el viernes anterior). Excepciones: las de cada calendario.
4. **Zona horaria:** las horas se escriben en hora del Este como dice la fuente y se pasan a UTC con `America/New_York` (`zoneinfo`), **nunca con un desfase fijo**. 10:30 ET es 15:30 UTC en invierno y 14:30 UTC en verano. Los tests cubren las semanas de cambio de hora.
5. **Si la API trae una semana nueva antes de su hora de publicación** según el calendario, la ingesta **falla** en vez de guardarla: significa que el calendario leído está mal.

#### 3.2 Histórico (`released_at` de las semanas pasadas)

1. **WPSR desde agosto de 2011:** la **fecha** sale del índice oficial de informes viejos, que lista cada publicación con su fecha y la semana que cubre.
   - Si la fecha es el **miércoles normal** (fin de semana de datos + 5 días): **10:30 ET**.
   - Si es **cualquier otra fecha** (feriado, atraso): el **final de ese día, 23:59:59 ET**. El índice no da la hora, así que se toma el lado seguro, como en el ADR 0016 §4.
   - El `robots.txt` de la EIA excluye esa página para robots. Por eso **no la lee un script**: el dueño la guarda desde el navegador, o un agente la lee con su herramienta web, una vez, como en el ADR 0016 §1.3. El parser lee el HTML guardado.
2. **WPSR antes de agosto de 2011:** no hay fecha de publicación documentada. Esas semanas se guardan para mostrarlas, pero **no entran al backtest** (ADR 0016 §4: si no hay forma documentada de saber cuándo se conoció, queda fuera).
3. **WNGSR:** no se encontró una lista oficial de fechas pasadas. Las semanas anteriores a la primera lectura en vivo se guardan para mostrarlas, pero **no entran al backtest**, salvo que el Issue de la ingesta encuentre y cite una fuente oficial de esas fechas.
4. **No se adivina con una regla fija.** "Miércoles 10:30" falla en las semanas con atrasos (por ejemplo, la de noviembre de 2023), y esas son justo las semanas raras que un estudio no puede tomar por buenas.

#### 3.3 Primer dato y dato revisado

1. Como en el ADR 0018 §2.3: **`first`** es el valor leído después de `released_at` y **antes de la publicación siguiente del mismo informe**. Cualquier otro es **`revised`**.
2. **Todo lo que se carga del histórico por la API es `revised`**: la API da el valor vigente, no el primero.
3. **WPSR:** las revisiones son raras y salen en el informe siguiente o en la hoja de erratas. **WNGSR:** las revisiones de 4 Bcf o más salen en el informe siguiente, y las de 10 Bcf o más **fuera de calendario** a las 2:00 p.m. ET. Una publicación fuera de calendario es una fila más del calendario, con su hora.
4. Un estudio que necesite el primer dato del pasado puede usar el archivo de revisiones del WNGSR (desde noviembre de 2015) y la hoja de erratas del WPSR para saber qué semanas cambiaron. Eso se decide en el pre-registro de ese estudio, no acá.

### 4. Lectura respetuosa de la API

1. **En vivo:** el día de cada informe, la primera lectura va a las **10:31 ET** (o a la hora de la excepción + 1 minuto). Si la semana nueva todavía no está, se reintenta a los 10, 30, 90 y 150 minutos. Después, aviso (`ops_event`) y se espera al día siguiente.
2. **Un pedido por serie y por lectura**, uno por vez, con **al menos 1 s** entre pedidos. Son 8 series: menos de 50 pedidos por semana.
3. **Histórico:** una sola carga, un pedido por serie (cada serie semanal tiene menos de 5 000 filas). Se repite solo si cambia algo, con su Issue.
4. Ante una **suspensión de la clave, 403 o 429**, el comando se frena, avisa y **no** reintenta hasta la lectura siguiente. Nunca se esquiva un límite.
5. **User-Agent propio** de cliente automático, sin datos personales, como en `config/cot.toml`.

### 5. Contratos, tablas y módulo (a definir en el Issue de la ingesta)

Contratos primero (`CLAUDE.md` §1): se definen en el PR de la ingesta o en uno anterior, no en este.

| Pieza | Nombre propuesto | Qué es |
|---|---|---|
| Contrato | `contracts/eia_release.v1.schema.json` | Una publicación de un informe: `report` (`wpsr`, `wngsr`), `period_end` (el viernes de fin de la semana), `status` (`scheduled`, `rescheduled`, `out_of_cycle`), `scheduled_at` y `published_at` (desde cuándo se sabía) en UTC terminados en `Z`, `time_rule` (`release_page`, `schedule_page`, `archive_date_standard`, `archive_date_end_of_day`) y `source_url` |
| Contrato | `contracts/eia_weekly_value.v1.schema.json` | Un valor de una serie: `series_id`, `period_end`, `value` como string decimal exacto (nunca `float`), `unit`, `vintage` (`first`, `revised`), `released_at`, `retrieved_at` y `source_url` sin la clave |
| Tabla | `eia_releases` | Solo agregar. Clave natural: (`report`, `period_end`, `status`, `scheduled_at`) |
| Tabla | `eia_weekly_values` | Solo agregar. Clave natural: (`series_id`, `period_end`, `vintage`, `value`) |
| Config | `config/eia.toml` | URL base, User-Agent, series (id, informe y unidad), horas normales de cada informe, horarios de reintento, URLs de los calendarios |
| Módulo | `intelligence/cfds_intel/energy` (Python, nuevo) | Ingesta. Se agrega al Registro de módulos en el PR de la ingesta |
| Variable | `EIA_API_KEY` en `.env` (Mac mini) y en `.env.example` con valor falso | §2 |
| Migraciones | Las próximas libres al momento de la ingesta | Coordinar el número con los PRs abiertos |

**Por qué no `official_release.v1` y `release_actual.v1`** (ADR 0016 y 0018): sus `source` y `series` son listas cerradas (`fed_fomc`, `bls`), y el período de referencia de `release_actual.v1` es un mes (`AAAA-MM`), no una semana. Sumar la EIA ahí es un `v2` que obliga a migrar a sus consumidores. Dos contratos nuevos no rompen nada. **Si el dueño prefiere un solo contrato de publicaciones oficiales**, se hace el `v2`: lo elige al aprobar.

**Tamaño:** 8 series semanales, unas 2 300 filas por serie desde 1982 y 52 por año después. Menos de 10 MB.

**Quién escribe y quién lee:** escribe Python (`cfds_intel.energy`). Cualquier cálculo sobre los datos (cambio semanal, comparación con el promedio de 5 años, sorpresa contra lo esperado) se define en otro Issue, en el lenguaje que corresponda según `CLAUDE.md` §1. La UI **no calcula**: muestra lo que le manda el gateway, con su propio contrato, en otro Issue.

### 6. Cita y uso

1. Toda vista o exportación dice "Fuente: U.S. Energy Information Administration (<fecha de publicación>)", como pide la EIA. **Sin el logo** de la EIA.
2. **No se alteran los valores.** Todo lo que se calcule a partir de ellos se muestra como **cálculo nuestro**, no como dato de la EIA (términos de la API).
3. Es **contexto**: no suma al puntaje hasta que un estudio pre-registrado lo valide (ADR 0003 §C). Ningún texto lo presenta como señal ni promete ganancias.
4. **Solo en el Mac mini:** la ingesta se niega a guardar si no está `CFDS_DATA_MACHINE` (como las cargas de IG). En la nube, solo tests con números inventados.

## Alternativas consideradas

- **Archivos masivos de la API** (sin clave): sirven para el histórico, pero se actualizan a las 5 a.m. y a las 3 p.m. ET, así que el dato del miércoles llega horas después de las 10:30. Quedan como **plan B para el histórico** si el dueño prefiere no registrarse.
- **Las tablas CSV de `ir.eia.gov`** (sin clave, a las 10:30): darían el primer dato sin clave, pero su formato cambia desde el 10-6-2026 y son otro camino para el mismo dato. Quedan como **plan B para el vivo**, con su Issue, si el dueño no crea la clave.
- **FRED o ALFRED** (Fed de St. Louis): prohíben guardar sus datos (`docs/research/fuentes-datos-gratis-2026-11.md`). Descartado.
- **Proveedores pagos** (Bloomberg, Refinitiv): de pago y con licencia. Va contra pagar solo cuando hace falta.
- **Regla fija "miércoles 10:30" para todo el histórico:** falla en las semanas con atrasos. Descartada (§3.2.4).

## Riesgos y consecuencias

- **Hora exacta del histórico:** el índice de informes viejos da la fecha, no la hora. Las semanas normales usan 10:30 ET, que es la hora documentada. Las raras usan el final del día, del lado seguro. Si un miércoles normal se hubiera publicado más tarde sin quedar documentado, el dato se usaría unas horas antes. Es un riesgo chico, y el estudio que use la hora exacta lo tiene que decir.
- **Valores revisados en el histórico:** la API da el valor vigente. Para la mayoría de las semanas es el mismo que el primero, pero no siempre. Por eso el histórico es `revised` (§3.3).
- **Límites de la API sin número publicado:** se leen muy pocos datos por semana. Si igual suspenden la clave, el comando se frena y avisa.
- **La EIA cambia formatos y fechas:** si cambia un id de serie, una URL o el calendario, la ingesta falla con error en vez de adivinar.
- **Datos de EE. UU. solamente:** no hay inventarios de Europa ni de Asia. El panel lo dice.
- **Cierres del gobierno de EE. UU.:** pueden atrasar o suspender publicaciones. El calendario guarda lo que se publicó y cuándo. Una semana sin publicación queda vacía, nunca interpolada.

## Pendiente del dueño al aprobar

1. **Aprobar o no** la EIA como fuente (este ADR).
2. **Crear la clave** (registro con su email y aceptación de los términos de la API) y ponerla en el `.env` del Mac mini como `EIA_API_KEY`. Hasta que exista, la ingesta no corre.
3. **Series:** la lista de §"Series" (recomendada) u otra.
4. **Producción de gas natural:** fuera de v1 porque es mensual (recomendado) o con su propio Issue.
5. **Contratos:** dos contratos nuevos (recomendado) o un `v2` común con `official_release.v1` y `release_actual.v1`.
6. **Índice de informes viejos del WPSR:** guardarlo él desde el navegador o dejar que un agente lo lea una vez con su herramienta web.

Aprobado este ADR, se abre el Issue de la ingesta: contratos, migraciones, `config/eia.toml`, el módulo, `.env.example`, tests (semanas con feriado, atraso, publicación fuera de calendario, cambio de hora, clave ausente, clave borrada de los logs, semana nueva antes de hora) y la carga en el Mac mini.
