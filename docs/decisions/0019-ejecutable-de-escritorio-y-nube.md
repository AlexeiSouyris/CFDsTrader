# 0019 – Ejecutable de escritorio e interfaz online con la nube

- **Estado:** propuesta
- **Fecha:** 2026-11-26
- **Aprobada por:** pendiente. Solo el dueño la aprueba (modo de un solo constructor, `CLAUDE.md`); un agente no la da por aprobada. **Nada de este ADR habilita dinero real ni órdenes reales.**
- **Issue:** #712 (épica #707)
- **Relacionados:** [ADR 0001](0001-arquitectura-hibrida.md) (arquitectura), [ADR 0014](0014-app-celular-cuentas-push-votos.md) (cuentas, push y acceso por Tailscale, en propuesta), [ADR 0002](0002-broker-ig.md) (IG), [ADR 0009](0009-tiingo-verificacion-transitoria.md) (Tiingo), [ADR 0011](0011-historico-intradia-databento.md) (Databento), [ADR 0012](0012-tesoro-y-vix.md) (VIX), [ADR 0017](0017-ibkr-paper-futuros-micro.md) (IBKR), `docs/guides/puesta-en-marcha-local.md`, `infra/scripts/ui_liquidez.sh`, #708, #709 y #710 (los otros ejes de la épica)

## Contexto

Pedido del dueño (26-11-2026): «Si podemos hacer esta terminal un ejecutable que lo pueda correr en cualquier computadora y despues una interfaz online que se conecta a la cloud para subir datos y cosas.»

Son dos pedidos: (1) un **ejecutable** para correr la terminal en cualquier computadora y (2) una **interfaz online** conectada a la nube para subir datos. Este ADR compara opciones y propone un camino. **No implementa nada, no crea cuentas y no gasta.** Cada proveedor y cada costo queda **PENDIENTE** del dueño.

### Lo que existe hoy (26-11-2026, `origin/main` y el Mac mini)

| Pieza | Estado |
|---|---|
| UI (`ui/`) | React + Vite. Solo corre con el servidor de desarrollo de Vite, cuyo proxy manda `/api` al gateway y agrega del lado del servidor el token anti-CSRF `x-cfds-token` (#292). Ya es una PWA instalable (`manifest.webmanifest` y `sw.js`, #307). **No hay servidor de producción** (pendiente también en el ADR 0014, Consecuencias, punto 4). |
| Gateway (`cfds-gateway`) | Rust (hyper) en `127.0.0.1:8787` (`config/gateway.toml`). Lee PostgreSQL con `sqlx` y **no usa Redis**. El binario de debug pesa 62 MB. |
| PostgreSQL 16 y Redis 7.4 | `infra/docker-compose.yml` con Docker Desktop, solo en `127.0.0.1`. Redis lo usan la ingesta (`cfds-ingest`, `cfds-storage`) y el flujo de noticias de Python. |
| Esquema | 49 migraciones en `infra/migrations/`. **46 usan rasgos propios de PostgreSQL** (`TIMESTAMPTZ`, `JSONB`, triggers en PL/pgSQL como los de solo agregar). |
| `intelligence/` | Python 3.13 con `uv`. El entorno ocupa 965 MB (torch: 565 MB). Las noticias se interpretan con Ollama y `Ministral-3-8B` Q4_K_M (ADR 0008); la carpeta de modelos de Ollama ocupa 10 GB en el Mac mini. |
| Brokers | IG demo por REST, con usuario, contraseña y API key en `.env`; el cupo de 10.000 puntos por semana es **de la cuenta**, no de la máquina. IBKR paper por IB Gateway en `127.0.0.1:4002`, con login manual del dueño (ADR 0017). |
| Arranque | A mano: Git, Docker Desktop, Rust, uv, Node y el `.env` (`docs/guides/puesta-en-marcha-local.md`), más `infra/scripts/ui_liquidez.sh`, que compila el gateway y abre Vite. |
| Credenciales | Texto plano en el `.env` de cada máquina (fuera del repo). |
| Repo | **Público** en GitHub. Por eso nunca entran datos ni secretos. |
| `bolsa` | La terminal de texto (`cfds-terminal`) ya es un binario único de Rust. |

Tamaño de la base del Mac mini, medido el 26-11-2026 con `pg_total_relation_size` (solo lectura):

| Tabla | Tamaño | Filas | Origen de los datos |
|---|---|---|---|
| **Base completa** | **7,3 GB** | | |
| `futures_bars_1m` | 6,2 GB | 25,0 M | Databento |
| `futures_candles` | 465 MB | 2,2 M | armadas en Rust desde Databento |
| `emotional_states` | 212 MB | 494 k | propias, calculadas sobre precios |
| `liquidity_levels` y `liquidity_level_events` | 139 MB | 874 k | propias, sobre velas de IG y de futuros |
| `ibkr_bars` | 39 MB | 214 k | IBKR |
| El resto (unas 40 tablas) | menos de 100 MB cada una | | |

Sin `futures_bars_1m`, que solo sirve para rearmar `futures_candles`, la base pesa **≈ 1,1 GB**.

### Lo verificado en fuentes oficiales (26-11-2026)

**Escritorio:**

| Tema | Dato | Fuente |
|---|---|---|
| Tauri 2 | Apps para "Linux, macOS, Windows, Android and iOS" con el motor web del sistema (WKWebView, WebView2, WebKitGTK). La licencia del framework no se releyó hoy: **a confirmar** en su repositorio | [tauri.app](https://tauri.app/) |
| Sidecar de Tauri | `bundle.externalBin` en `tauri.conf.json`; un binario por plataforma con el sufijo del *target triple*; permiso `shell:allow-spawn` con `"sidecar": true` | [Sidecar](https://v2.tauri.app/develop/sidecar/) |
| Actualizador de Tauri | Firma propia **obligatoria** ("This cannot be disabled"), con un par de claves aparte de la firma del sistema. Los archivos pueden ir en un JSON estático (por ejemplo GitHub Releases). TLS obligatorio en producción | [Updater](https://v2.tauri.app/plugin/updater/) |
| Firma en macOS | Apple Developer Program: "The Apple Developer Program annual fee is 99 USD". Con la cuenta gratis no se notariza y la app "will still show up as not verified". La firma *ad hoc* (`-`) existe, pero macOS igual pide permitir la app a mano en Privacidad y Seguridad | [Apple](https://developer.apple.com/support/enrollment/), [Tauri macOS](https://v2.tauri.app/distribute/sign/macos/) |
| Firma en Windows | "It is not required to execute your application on Windows". Sin firma, SmartScreen advierte en las descargas del navegador, y Smart App Control de Windows 11 "blocks unknown unsigned apps". Desde 2024 un certificado EV ya no da reputación inmediata | [Tauri Windows](https://v2.tauri.app/distribute/sign/windows/) |
| Azure Artifact Signing (antes Trusted Signing) | Plan Basic con 5.000 firmas por mes. **USD 9,99/mes** según la página de precios de Microsoft vista por el buscador; leída directo, la página no mostró el número. Si sirve para una persona y desde España: **a confirmar** | [Precios](https://azure.microsoft.com/pricing/details/artifact-signing/) |
| PostgreSQL embebido | Crate `postgresql_embedded` (Apache-2.0 o MIT): baja PostgreSQL al ejecutar, o al compilar con la *feature* `bundled`. Linux, macOS y Windows | [theseus-rs/postgresql-embedded](https://github.com/theseus-rs/postgresql-embedded) |
| PGlite | PostgreSQL compilado a WASM, "under 3MB Gzipped". La página no dice si otro proceso se puede conectar por el protocolo de PostgreSQL | [pglite.dev](https://pglite.dev/) |
| Licencia de Redis | 7.4: RSALv2 o SSPLv1. Desde Redis 8 se suma AGPLv3 | [redis.io](https://redis.io/legal/licenses/) |
| Docker Desktop | Plan Personal gratis, 1 usuario. La página no trae las condiciones de uso comercial: **a confirmar** en su FAQ | [docker.com/pricing](https://www.docker.com/pricing/) |

**Nube** (precios en USD sin impuestos):

| Proveedor | Plan gratis | Primer plan pago | Fuente |
|---|---|---|---|
| Supabase (PostgreSQL + Auth) | 500 MB de base, 1 GB de archivos, 5 GB de egress, 50.000 usuarios activos por mes, 2 proyectos. **Se pausa después de 1 semana sin actividad. Sin backups** | Pro desde **25/mes**: 8 GB de disco, backups diarios por 7 días, 10 de crédito de cómputo. PITR aparte, 100/mes | [Pricing](https://supabase.com/pricing) |
| Supabase, seguridad | Las APIs "accept only HTTPS connections using TLS 1.2 or TLS 1.3". En PostgreSQL el SSL es opcional por defecto y se puede exigir. Cifrado en reposo AES-256 con claves por proyecto en HSM y SOC 2 Type 2 (página de seguridad y FAQ de residencia de datos, vistas por el buscador; **releer** al decidir) | [Platform security](https://supabase.com/docs/guides/security/platform-security), [Security](https://supabase.com/security), [Data residency FAQ](https://supabase.com/legal/privacy-resources/data-residency-and-transfers-faq) |
| Cloudflare Pages (sitio estático) | 500 builds por mes, 20.000 archivos, 25 MiB por archivo | — | [Límites](https://developers.cloudflare.com/pages/platform/limits/) |
| Cloudflare Workers y D1 (SQLite) | Workers: 100.000 pedidos por día, 10 ms de CPU por pedido. D1: 5 GB en total, 5 M filas leídas y 100.000 escritas por día | Workers Paid: mínimo **5/mes** | [Workers pricing](https://developers.cloudflare.com/workers/platform/pricing/) |
| Cloudflare R2 (archivos) | 10 GB-mes, 1 M operaciones de clase A y 10 M de clase B, egress gratis | 0,015 por GB-mes | [R2 pricing](https://developers.cloudflare.com/r2/pricing/) |
| Cloudflare Access | Gratis hasta 50 usuarios | — | ADR 0014 (leído el 24-11-2026) |
| Neon (PostgreSQL) | 1 GB por proyecto, 100 CU-hora por mes, se apaga a los 5 min, restauración de 6 h | Launch por uso (0,106 por CU-hora y 0,35 por GB-mes; Neon estima ~15/mes para algo chico) | [Pricing](https://neon.com/pricing) |
| Fly.io | 1 de crédito sin tarjeta, o 5 de uso por mes con tarjeta, hasta 3 máquinas | Máquina `shared-cpu-1x` de 256 MB: 2,19/mes; volumen 0,15 por GB-mes; PostgreSQL gestionado desde 4/mes | [Pricing](https://docs.fly.io/about/pricing) |
| Hetzner (servidor propio) | — | Desde **€ 5,49/mes sin IVA**. La página no mostró CPU, RAM ni tráfico | [Cloud](https://www.hetzner.com/cloud/) |
| Tailscale | Personal gratis hasta 6 usuarios, solo uso no comercial | — | ADR 0014 (leído el 24-11-2026) |

### Licencias de los datos: qué puede salir del Mac mini

Este es el límite más duro. Copiar datos de mercado a la nube, o mostrarlos online, puede violar los términos de la fuente aunque nadie más los vea.

| Fuente | Tablas | Qué se sabe de sus términos | ¿A otra computadora del dueño? | ¿A una nube privada? | ¿Visible para otras personas? |
|---|---|---|---|---|---|
| **IG** (API demo) | `candles`, `candle_spreads`, `daily_candles`, `sentiment_observations` y lo calculado sobre ellas | **No leídos.** `labs.ig.com` respondió 500 y 404, y el buscador no trajo el acuerdo de la API. La guía de puesta en marcha ya dice que los datos de IG quedan en la base local | PENDIENTE | **No**, hasta leerlos | **No** |
| **IBKR** | `ibkr_bars`, `ibkr_bar_corrections` | Datos de bolsa bajo acuerdos de suscriptor. "No profesional" es uso personal, no de negocio, y los acuerdos de bolsa (por ejemplo, el de NYSE) limitan la retransmisión. El de CME no se leyó | Solo con su propio IB Gateway | **No** | **No** |
| **Databento** | `futures_bars_1m`, `futures_bars_1d`, `futures_candles` y lo calculado sobre ellas (instrumentos `.FUT`) | El blog oficial dice que el histórico (más de 24 h) no pide licencia para acceder, pero sí puede pedirla si "You plan to redistribute the data" ([Databento](https://databento.com/blog/introduction-market-data-licensing)). La licencia de uso personal sigue sin encontrarse (ADR 0011, pregunta abierta 1) | PENDIENTE (copia de respaldo propia) | **No** | **No** |
| **Cboe** (VIX) | `vix_daily` | Los términos prohíben guardarlo. Está solo en el Mac mini por decisión del dueño, y el ADR 0012 §2 dice que **nunca** va "a un servicio externo" | **No** (ADR 0012) | **Nunca** | **Nunca** |
| **Tiingo** | ninguna: no se guarda nada (ADR 0009) | — | — | — | — |
| **Forex Factory** | `economic_events` (forecast y previous) | El dueño leyó que se pueden guardar para uso personal (ADR 0018) | Sí | PENDIENTE | No |
| **Fed, BLS, CFTC, Tesoro, NY Fed** | `official_releases`, noticias de la Fed, COT, `treasury_yields`, EFFR y SOFR | Dominio público, o uso libre citando la fuente (ADR 0004, 0006, 0012, 0015 y 0016) | Sí | Sí, con la cita | Sí, con la cita |
| **BCE** | noticias del BCE, €STR, `fx_rates` | Uso libre si se cita al BCE y no se altera, diciendo cuándo se transforma (ADR 0006) | Sí | Sí | Sí, con la cita y la marca "interpretado" |
| **Propios** | métricas de `backtest_runs`, `walk_forward_reports`, `analog_reports`, el registro de resultados, el diario, el progreso de la academia y las specs de estrategias | Sin licencia de terceros. El registro de resultados **ya es público** en el repo | Sí | Sí | Lo decide el dueño |
| **Propios con precios adentro** | `proposals` (entrada y stop), `backtest_run_curves`, `emotional_states`, `liquidity_*` | Son números derivados de precios licenciados. Una serie de precios es redistribuir; una métrica agregada no. Dónde está el límite exacto: PENDIENTE (términos de IG y de Databento) | Sí | **Solo métricas agregadas**, hasta aclararlo | No |

**Un riesgo que aparece para el ADR 0014:** mostrar precios de IG o de IBKR a los otros miembros por la tailnet puede ser "dar los datos a terceros", aunque no haya nube. Queda como pregunta para el dueño (punto 8 de abajo).

## Decisión (propuesta)

### 1. Tres ediciones, no una

Correr **todo** en cualquier computadora choca con cuatro cosas: (a) los datos se bajan de nuevo, y Databento cobra cada descarga; (b) el cupo de IG es de la cuenta, así que dos máquinas cargando gastan el mismo cupo; (c) IB Gateway corre con el login manual del dueño y, por lo que se sabe, IBKR corta la sesión anterior cuando el mismo usuario entra desde otro lado (**a confirmar** en la documentación oficial); (d) las licencias de la tabla de arriba. Por eso se proponen tres ediciones:

| Edición | Para qué | Qué trae | Datos | Credenciales de broker |
|---|---|---|---|---|
| **Completa** (la máquina de datos; hoy, el Mac mini) | Ingesta, IB Gateway, Ollama, investigación | Todo lo de hoy | La base completa | Sí, en el llavero del sistema (punto 2.4) |
| **Cliente** (cualquier computadora) | Usar la terminal en otra máquina o fuera de casa | Solo la UI | **Ninguno local:** lee el gateway del Mac mini por la tailnet (ADR 0014 §4) | **Ninguna** |
| **Portátil sin conexión** (opcional, más adelante) | Estudiar y probar estrategias sin red | Gateway y PostgreSQL embebido | Un subconjunto de ~1,1 GB restaurado de un respaldo propio, solo de las fuentes que lo permitan | Ninguna |

La regla `CFDS_DATA_MACHINE=1` (#143) no cambia: **una sola máquina carga datos.**

### 2. Ejecutable de escritorio

#### 2.1 Opciones

| | (a) App de Tauri con el gateway como sidecar | (b) Paquete de Docker con compose | (c) PWA servida por el gateway local |
|---|---|---|---|
| Qué instala el usuario | Un `.dmg`, `.msi` o `.AppImage`/`.deb` | Docker Desktop y un `compose.yml` | Un binario del gateway y la carpeta de la UI compilada; la PWA se instala desde el navegador |
| Encaje con el proyecto | Bueno: Tauri es Rust, como el motor | Es lo que ya se usa para PostgreSQL y Redis | Es lo que ya existe (PWA de #307); falta que el gateway sirva la UI |
| UI | Tres motores web distintos (WKWebView, WebView2 y WebKitGTK): hay que probarla en los tres | El navegador del usuario | El navegador del usuario |
| PostgreSQL y Redis | Docker, o PostgreSQL embebido en la portátil; Redis no hace falta en cliente ni portátil | Contenedores, como hoy | Docker, como hoy |
| Python | No se empaqueta (ver 2.3) | Imagen propia; en macOS los contenedores no usarían la GPU de Apple (no verificado hoy), así que Ollama conviene nativo | No se empaqueta |
| Firma | Hace falta para no tener advertencias (ver 2.5) | No firma una app: corre imágenes | El binario del gateway tiene el mismo problema de firma que (a) |
| Actualización | Actualizador de Tauri con firma propia, gratis | `docker compose pull` | Reemplazar el binario y la carpeta |
| Credenciales | Llavero del sistema desde Rust; la UI nunca las ve | Archivos `.env` o *secrets* de Docker | Llavero del sistema desde el gateway |
| Trabajo | El más alto: proyecto nuevo, compilación por plataforma, pruebas en tres motores | Medio: imágenes del gateway, la UI y `intelligence` | El más bajo |
| Lo malo | Más piezas y costos de firma | Docker Desktop pesa y corre una máquina virtual en macOS; para "cualquier computadora" es pedir mucho | Es una pestaña del navegador, no una app |

#### 2.2 Qué se propone

- **Fase 1 (c):** el gateway sirve la UI compilada (`ui/dist`) y agrega él mismo el token `x-cfds-token`, como hoy hace el proxy de Vite. Es el mismo "servidor de producción" que ya pide el ADR 0014. Más un **script de instalación de un comando** que reemplaza los pasos a mano de la guía. PostgreSQL y Redis siguen en Docker.
- **Fase 2 (a):** Tauri, **solo después de una prueba corta (spike)** que demuestre que la UI se ve igual en los tres motores web y que el gateway arranca y se apaga bien como sidecar. Primero la **edición cliente** (solo la UI apuntando al gateway del Mac mini), que no tiene datos ni credenciales; después la completa.
- **(b)** queda como hoy: la forma de correr PostgreSQL y Redis en la edición completa, no el producto.

#### 2.3 Base de datos y Python

- **PostgreSQL:** se queda en todas las ediciones que tengan datos. En la portátil, `postgresql_embedded`, con **las mismas migraciones y el mismo SQL**.
- **SQLite para una edición portátil: descartado.** 46 de las 49 migraciones usan rasgos de PostgreSQL. Habría que reescribir el esquema y cada consulta: sería una segunda implementación del almacenamiento, contra la regla de que cada cosa vive en un solo lugar.
- **Tablas para la portátil:** todas menos `futures_bars_1m` (6,2 GB), `vix_daily` (ADR 0012), `ops_events` y las de fuentes cuya licencia no lo permita (tabla de licencias). Hoy serían unos 1,1 GB.
- **Redis:** no hace falta en la edición cliente ni en la portátil, porque el gateway no lo usa. Si algún día se distribuye un binario de Redis dentro del instalador, se revisa la licencia: Redis 8 con AGPLv3, o Valkey (**no verificado**).
- **Python (`intelligence/`):** solo en la edición completa, con `uv` como hoy. **No se empaqueta:** torch ocupa 565 MB, el modelo de noticias varios GB y conviene la GPU. Las otras ediciones leen los resultados que ya están guardados. La portátil no interpreta noticias.

#### 2.4 Credenciales

- Pasan del `.env` al **llavero del sistema** (Llavero de macOS, Administrador de credenciales de Windows, Secret Service de Linux). Las lee el lado de Rust al arrancar; la UI nunca las ve. La librería se elige en su Issue (candidata: el crate `keyring`, **no evaluado**).
- El `.env` sigue sirviendo para desarrollo y CI.
- **Nunca** van al instalador, al repo, a los logs ni a la nube. Un chequeo de CI verifica que el paquete no tenga `.env`, datos ni modelos.
- La edición cliente no tiene credenciales de broker: solo la identidad de Tailscale y la sesión de la app (ADR 0014).

#### 2.5 Firma y actualización

| Caso | Qué hace falta | Costo |
|---|---|---|
| Solo las computadoras del dueño | macOS: firma *ad hoc* y permitir la app a mano una vez. Windows: sin firma (SmartScreen advierte; Smart App Control puede bloquearla). Linux: AppImage o `.deb` | **0** |
| Darle el ejecutable a otras personas (los miembros) | Apple Developer Program (Developer ID y notarización) y un certificado de Windows | USD 99/año, más Windows: Azure Artifact Signing USD 9,99/mes (**a confirmar**) o un certificado OV (precio **no verificado**). **PENDIENTE** |
| Actualizaciones | Actualizador de Tauri con su clave propia; los archivos en GitHub Releases del repo, que ya es público y no llevan datos ni secretos | 0 |

- La clave privada del actualizador y los certificados son del dueño. Si se pierde la clave del actualizador, las instalaciones no se actualizan solas y hay que reinstalar.
- Compilar para las tres plataformas pide *runners* de macOS, Windows y Linux; hoy el CI usa solo `ubuntu-latest`. Si son gratis en un repo público: **a confirmar** en la documentación de GitHub.

#### 2.6 IB Gateway e IG en cada máquina

- Solo la edición completa se conecta a los brokers.
- IB Gateway corre donde corre el motor, con el login manual del dueño y las guardas del ADR 0017 sin cambios (puerto, cuenta paper, kill switch).
- Las credenciales de IG viven solo en la edición completa. Una segunda máquina con esas credenciales gastaría el mismo cupo semanal.

### 3. Interfaz online y nube

#### 3.1 Primero, sin nube: el Mac mini por Tailscale

La "interfaz online" puede ser la misma UI servida por el Mac mini con `tailscale serve`, como propone el ADR 0014 §4: **0 €**, nada expuesto a Internet y **ningún dato licenciado sale del Mac mini**. Lo malo: el Mac mini tiene que estar prendido y cada dispositivo necesita la app de Tailscale.

#### 3.2 Después, nube solo para lo propio

Para lo que tiene que verse con el Mac mini apagado, o cargarse desde el celular (diario, progreso de la academia, borradores de estrategias), se propone una réplica en la nube **solo con datos propios y públicos**.

**Recomendación (PENDIENTE):** Supabase gratis (PostgreSQL, Auth y seguridad por fila, RLS) para los datos, y la UI estática en Cloudflare Pages gratis. Es PostgreSQL como el nuestro (no hace falta otro esquema en otro motor), trae autenticación con políticas por fila y cuesta 0 € dentro de sus límites. Las alternativas, abajo.

**Qué sube** (lista blanca en `config/sync.toml`, validada al arrancar):
- resultados de investigación: métricas de `backtest_runs`, `walk_forward_reports`, `analog_reports` y el registro de resultados;
- el diario (#615, #616), el progreso de la academia (#709) y las specs de estrategias (#710);
- datos públicos con su cita (Fed, BCE, BLS, CFTC, Tesoro);
- curvas y propuestas con precios, **solo como métricas agregadas** hasta aclarar las licencias.

**Qué nunca sube:**
- ninguna credencial ni clave: las de IG, el ID de la cuenta paper de IBKR, las claves de Databento y Tiingo, los tokens de Telegram, `CFDS_GATEWAY_TOKEN`, las contraseñas de PostgreSQL, las claves VAPID y los secretos TOTP (ADR 0014), y la clave del actualizador;
- datos de mercado licenciados: velas, spreads y sentimiento de IG; velas de IBKR; barras y velas de Databento y lo armado sobre ellas; el VIX;
- `ops_events` (pueden tener detalles internos).

Un test de CI falla si una tabla licenciada aparece en la lista blanca.

**Modelo de sincronización:**
- **Solo de salida desde el Mac mini:** sube lo del motor y baja lo creado online. Ningún puerto abierto; la nube no puede llegar al motor.
- **Solo agregar:** cada fila con un UUID, `created_at` en UTC y `origin` (`mac-mini` o `nube`). Editar es agregar una versión nueva y borrar es una marca. Así no hay conflictos.
- **Una spec que viene de la nube es un borrador.** La valida el motor en Rust (`strategy_spec.v1` de #710), y una corrida confirmatoria pide el pre-registro en el Mac mini. **Nada que baje de la nube cambia `config/`**: el sistema nunca cambia sus propios parámetros.
- **Contratos primero:** `sync_*.v1` en `contracts/` antes del código. El sincronizador vive en un solo lugar, en Rust (nombre y crate en su Issue).

**Autenticación:** un solo usuario, el dueño. Registro abierto **apagado**, la cuenta del dueño con un segundo factor TOTP (si Supabase Auth lo da en el plan gratis: **a confirmar**) y RLS que niega todo por defecto, con políticas `owner_id = auth.uid()`. Como segunda capa opcional, Cloudflare Access delante del sitio. Si se suman miembros, se usan las cuentas del ADR 0014; la nube sigue siendo solo del dueño salvo un ADR nuevo.

**Seguridad:**
- TLS en todo, con SSL **obligatorio** en la conexión a PostgreSQL.
- RLS en cada tabla, con un test que la verifica.
- Cifrado en reposo del proveedor (AES-256 según Supabase). Cifrar además las notas del diario del lado del cliente: PENDIENTE.
- **Backups:** el plan gratis no tiene. El Mac mini es la fuente de verdad de lo que produce el motor, y cada noche baja un `pg_dump` de lo creado online, que queda en su copia local.
- **Auditoría:** `sync_log` de solo agregar en los dos lados, más los registros de Auth.
- Región en la UE (el dueño vive en España).
- En el navegador va solo la clave pública del proyecto (pensada para ser pública, protegida por RLS). El sincronizador usa un rol propio con permisos mínimos y su clave en el llavero del Mac mini.
- El plan gratis se pausa a la semana sin actividad. La sincronización diaria debería evitarlo; qué cuenta como actividad: **a confirmar**.

#### 3.3 Dinero real: sin cambios

Ninguna pieza de la nube guarda credenciales de broker ni puede llegar al motor o al broker. El `LiveBroker` sigue sin poder construirse y la fase 11 sigue bloqueada (`CLAUDE.md` §5). Aunque alguien tomara la nube, no podría operar desde ahí.

### 4. Camino por fases

| Fase | Qué | Costo por mes | Depende de |
|---|---|---|---|
| 1 | Instalación de un comando (macOS y Linux; Windows después). El gateway sirve la UI compilada y se instala como PWA desde `127.0.0.1`. Credenciales al llavero | **0 €** | Este ADR |
| 2 | Tauri: primero la edición cliente, después la completa con el gateway como sidecar | **0 €** sin firma. Con firma: USD 99/año de Apple (~USD 8,25/mes) y Windows ~USD 9,99/mes (a confirmar). **PENDIENTE** | Spike y su ADR |
| 3a | Interfaz online: el Mac mini por Tailscale | **0 €** | ADR 0014 aprobado |
| 3b | Nube: Supabase gratis y Cloudflare Pages gratis, solo datos propios y públicos | **0 €**. Si no alcanza: Supabase Pro USD 25/mes, y Workers USD 5/mes si hiciera falta. **PENDIENTE** | ADR del proveedor de nube; licencias leídas |
| 4 (opcional) | Edición portátil sin conexión, con PostgreSQL embebido y ~1,1 GB | **0 €** | Términos de IG y Databento leídos |

**Costo mensual esperado del camino recomendado: 0 €.** Si el dueño quisiera todo firmado y la nube paga: unos **USD 43/mes** (25 de Supabase Pro, ~10 de la firma de Windows y ~8 de Apple). Un dominio propio es opcional (precio **no verificado**); Pages da un subdominio gratis.

### 5. ADRs y decisiones que vienen después

1. **ADR 0014** (cuentas y Tailscale) se decide antes de las fases 3a y 3b.
2. **ADR del empaquetado con Tauri** (librería principal nueva, `CLAUDE.md` §6), después del spike.
3. **ADR del proveedor de nube y la sincronización:** proveedor, región, lista blanca y contratos `sync_*.v1`.
4. **Actualizar los ADR 0002, 0011 y 0017** con lo que digan sus términos sobre copias en otra computadora y en la nube.
5. **Licencias de lo que se distribuya** dentro del instalador (Redis, PostgreSQL), si se distribuye.
6. **Edición portátil** (PostgreSQL embebido), si se hace.

## Alternativas consideradas

**Escritorio:**
- **Electron:** empaqueta su propio Chromium y Node (instaladores más pesados, otro runtime). Tauri reusa Rust y el motor web del sistema.
- **App nativa** (Swift, WinUI): una por sistema y otro lenguaje.
- **SQLite en la edición portátil:** descartada (punto 2.3).
- **PGlite (PostgreSQL en WASM):** pesa poco, pero no se verificó que el gateway en Rust se pueda conectar desde otro proceso. Se puede reconsiderar.

**Nube:**

| Opción | A favor | En contra | Costo |
|---|---|---|---|
| **Supabase + Cloudflare Pages (recomendada)** | PostgreSQL igual al nuestro, Auth y RLS incluidos | 500 MB; se pausa a la semana sin uso; sin backups en el plan gratis | 0 €; Pro USD 25/mes |
| Cloudflare Pages + Workers + D1 + R2 | Generoso y sin egress en R2; Access gratis | D1 es SQLite: otro esquema y otro motor; Workers es otro runtime | 0 €; Workers USD 5/mes |
| Neon (PostgreSQL) | PostgreSQL, se apaga solo, 1 GB | Sin Auth incluido: habría que sumar otro servicio | 0 €; ~USD 15/mes por uso |
| Fly.io (gateway en la nube) | Corre nuestro binario tal cual | Para servir datos necesita los datos, y ahí aparecen las licencias | Desde ~USD 2-6/mes |
| Servidor propio (Hetzner) | Control total | Una persona sola mantiene y parchea un servidor expuesto | Desde € 5,49/mes sin IVA |
| Todo el motor en la nube | Siempre prendido | Copia datos licenciados a un tercero (el VIX está prohibido); IB Gateway pide login manual; más superficie de ataque | Varía |
| **Sin nube, solo Tailscale** | 0 €, nada sale de casa | Depende del Mac mini prendido | 0 € |

## Consecuencias

- **Se vuelve más fácil:** instalar en una máquina nueva sin seguir la guía a mano; usar la terminal en otra computadora sin copiar datos ni credenciales; ver resultados y el diario con el Mac mini apagado (fase 3b).
- **Se vuelve más difícil:** mantener compilaciones para tres sistemas y la UI en tres motores web (fase 2); otra pieza que cuidar (la nube) con su RLS, sus backups y sus límites.
- **Riesgos:**
  - Subir por error una tabla licenciada. Mitigación: lista blanca, test de CI y la regla de "nunca" de arriba.
  - Que se filtre la clave del actualizador o un certificado. Mitigación: solo el dueño los tiene y nunca van al repo.
  - Cambios de precios y límites de los planes gratis: se releen antes de cada fase, con fecha.
  - Que el plan gratis se pause o se pierda: el Mac mini es la fuente de verdad y guarda backup de lo creado online.
- **Lo hace el dueño, no un agente:** crear cuentas (Supabase, Cloudflare, Apple, Microsoft), aceptar términos, pagar, generar claves y certificados, y leer los términos de IG, Databento e IBKR/CME. Ningún agente crea cuentas ni credenciales (`CLAUDE.md` §5).

## Lo que tiene que decidir el dueño (PENDIENTE)

1. ¿El camino por fases (1 → 2 → 3a → 3b, la 4 opcional) y las tres ediciones?
2. Fase 2: ¿Tauri después del spike, o alcanza con la PWA de la fase 1?
3. Firma: ¿Apple USD 99/año? ¿Windows (Azure Artifact Signing ~USD 9,99/mes, un certificado OV, o nada)? ¿Hace falta Windows?
4. Nube: ¿Supabase gratis + Cloudflare Pages gratis, otro proveedor, o ninguno (solo Tailscale)?
5. La lista de lo que sube, y si el diario y las propuestas pueden llevar precios sueltos.
6. Leer los términos de la API de IG, de Databento y de los datos de CME en IBKR sobre copias en otra computadora y en una nube privada.
7. Región de la nube (propuesta: UE) y si se cifran las notas del diario del lado del cliente.
8. Para el ADR 0014: si mostrar precios de IG o de IBKR a los otros miembros cuenta como darlos a terceros.
9. Dónde se compilan las versiones (en el CI o en el Mac) y quién guarda la clave del actualizador.
