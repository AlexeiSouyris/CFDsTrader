# 0017 – IBKR como segundo broker, solo en modo práctica (paper), para futuros micro (MNQ, MES)

- **Estado:** aceptada
- **Fecha:** 2026-11-24
- **Aprobada por:** el dueño (Alexei), el 24-11-2026, en el chat con su agente, textual: "apruebo el ADR 0017" (modo de un solo constructor, `CLAUDE.md`). Registrado en #530. Siguen pendientes del dueño: volver a pedir el permiso de futuros y decidir el depósito que abre la cuenta paper. **Este ADR no habilita dinero real**: eso sigue necesitando un ADR aprobado por dos personas (`CLAUDE.md` §5 y fase 11).
- **Issue:** #530
- **Actualización 25-11-2026 (etapas 2 y 3 en pausa):** IBKR negó el permiso de futuros porque el dueño no califica. La etapa 1 está hecha (#534, PR #535) y la prueba en vivo dio velas cerradas de 1h de MNQ y MES **sin** el permiso de futuros y **sin** suscripción de CME (#530). El dueño decidió usar IBKR **solo como fuente de datos gratis**, para ahorrar cupo de IG, textual: "Para ahorrar cupo en IG sirve. Entonces dejemoslo que por lo menos sirve para eso." Las etapas 2 (órdenes paper de MNQ) y 3 (comparar costos) quedan **en pausa** hasta que haya permiso de futuros. Sigue #536.
- **Relacionados:** [ADR 0002](0002-broker-ig.md) (IG, cuenta demo), [ADR 0015](0015-tasas-usd-nyfed-y-base-de-futuros.md) (base de los futuros), `docs/analysis/2026-11-programa-2-propuesta.md` §1.4, `docs/guides/consultas-proveedores.md` §3, #493 (comisiones de IBKR leídas), resultados R018 y R027 de `docs/analysis/registro-de-resultados.csv`

## Contexto

**Lo que tenemos hoy.** El único broker es IG con cuenta demo (ADR 0002). Operamos CFD de índices. Un CFD de índice de IG cobra, por cada noche que una posición queda abierta, la tasa de referencia **más una tasa administrativa de alrededor del 3 % anual** sobre el nominal (programa 2 §1.4).

**Por qué importa.** El programa 2 (#493) mostró que el costo de estar puede pesar más que cualquier señal:
- **R018:** comprar y mantener el CFD (cartera US500 + US100 + US30, 2013–2025) dio ≈ +1,1 % por mes **ya neto** de esa financiación. Un vehículo sin la tasa administrativa le sumaría del orden de **0,25 % por mes** con el mismo riesgo. Es una cuenta aproximada, no un backtest.
- **R027:** la hipótesis H1 del programa Nasdaq (deriva antes del FOMC en US100) **no pasa**. Ninguna señal intradía que probamos supera los costos del CFD. Para el Nasdaq, el instrumento natural sin tasa administrativa es el futuro **micro E-mini Nasdaq-100 (MNQ)**; para el S&P 500, el **micro E-mini S&P 500 (MES)**.

Un futuro no cobra la tasa administrativa: el costo de financiarse está dentro de su precio (la base = tasa − dividendos, ADR 0015). Pero nada de esto se puede medir bien sin precios y órdenes de un broker de futuros real, en modo práctica.

**La cuenta del dueño.** El dueño abrió una cuenta en **Interactive Brokers (IBKR)**: residente en España, plan **IBKR Pro**, tipo de cuenta **Cash** (dato del dueño, 24-11-2026, #530). En la solicitud, el permiso de **futuros apareció denegado** (#493). Hoy no es una vía disponible sin volver a pedirlo.

### Lo verificado en documentación oficial (24-11-2026)

| Dato | Qué dice | Fuente |
|---|---|---|
| Cuenta paper: permisos y suscripciones | Los permisos de trading, las suscripciones de datos de mercado, la moneda base y el resto de la configuración **son los mismos** que los de la cuenta real. Los precios simulados salen de precios y tamaños reales del mercado | [ibkrguides: About paper trading accounts](https://www.ibkrguides.com/clientportal/aboutpapertradingaccounts.htm) |
| Cuenta paper: depósito | La cuenta paper **no se abre hasta que la cuenta real tenga fondos** | misma página |
| Puertos de TWS | Sesión real: 7496. Sesión paper: 7497 | [TWS API: Initial Setup](https://interactivebrokers.github.io/tws-api/initial_setup.html) |
| Modo solo lectura de la API | Viene **activado** por defecto. Con él activado, la API no ve información de órdenes | misma página |
| IB Gateway frente a TWS | Para la API son idénticos | misma página |
| Puerto de IB Gateway en paper | **4002**. IB Gateway dice "Paper Trading (simulated trading)". Escucha en todas las interfaces (`*:4002`); el filtro a esta máquina lo hace la opción "Allow connections from localhost only" de IB Gateway, con "Trusted IPs" = `127.0.0.1` | Leído por el dueño en Configure → Settings → API → Settings, y puerto verificado abierto con `lsof` en la Mac (25-11-2026, #530) |
| Comisión de futuros micro (MNQ, MES) | **0,25 USD por contrato y por lado**, hasta 1.000 contratos por mes, más tasas de bolsa y regulatorias (no leídas) | [IBKR Irlanda, comisiones de futuros](https://www.interactivebrokers.ie/en/pricing/commissions-futures.php), leído por el dueño en el navegador (#493) |

### Lo que no se pudo verificar (queda **a confirmar**)

Las páginas de IBKR rechazaron la lectura automática (403) varias veces. Lo que sigue **no se usa como dato** hasta que una persona lo lea en la fuente oficial y anote la fecha:

| Dato | Lo que se encontró | Por qué queda a confirmar |
|---|---|---|
| Futuros en una cuenta **Cash** | Resúmenes de búsqueda citan FAQ de IBKR que dicen que **sí** se pueden operar futuros en una cuenta Cash (sin tasas de margen intradía y con el margen inicial cubierto con efectivo libre). Otra fuente no oficial dice que hace falta una cuenta Margin | La FAQ oficial devolvió 403. Además el permiso de futuros del dueño figura denegado |
| Puerto de IB Gateway en sesión real | Resúmenes de páginas de IBKR Campus: real 4001 (el de paper, 4002, ya está confirmado arriba) | La página de IBKR Campus devolvió 403 |
| Prefijo de las cuentas paper | Fuentes de terceros dicen que empiezan con `DU` | No hay fuente oficial leída |
| Costo de los datos de CME (no profesional) | Páginas oficiales con valores distintos (1,25 USD y 1,55 USD por mes para CME nivel 1) y una exención por comisiones que varía según la página | Hay que leer el valor actual en el gestor de suscripciones de la cuenta |
| Compartir datos de la cuenta real con la paper | La página oficial dice que las suscripciones son "las mismas"; no se leyó cómo se activa ni si hay límites de sesiones simultáneas | Página específica no encontrada |
| Tasas de bolsa y regulatorias de MNQ y MES | No leídas | Página aparte de IBKR y de CME |
| Especificación de MNQ y MES (valor por punto, tick, horarios, vencimientos) | No leída en CME | La página de CME no respondió a la lectura automática (programa 2 §1.4) |

## Decisión (propuesta)

**IBKR como segundo broker, solo con la cuenta paper**, para futuros micro (primero MNQ, después MES). IG sigue siendo el broker principal (ADR 0002). **Ninguna orden real**: el `LiveBroker` sigue sin poder construirse (`cfds-broker`, `live`).

### 1. Conexión

- **TWS API a través de IB Gateway**, corriendo en el **Mac mini**, escuchando solo en `127.0.0.1`.
- **El login lo hace el dueño a mano** en IB Gateway, eligiendo el modo paper. El proyecto **nunca** guarda el usuario ni la contraseña de IBKR, ni los pide, ni los escribe un agente.
- En `config/` van solo el host, el puerto y el `clientId`. Si hace falta, el ID de la cuenta paper va en `.env` (documentado en `.env.example` con un valor falso).

### 2. Guardas obligatorias (el adaptador se niega a arrancar si falla alguna)

1. **Puerto:** rechaza conectarse a los puertos de sesión real (7496 de TWS y, cuando se confirme, 4001 de IB Gateway). La lista de puertos prohibidos va en la configuración validada al arrancar.
2. **Cuenta:** al conectar, la API informa las cuentas de la sesión. Si **alguna** no es paper, el adaptador se desconecta, activa el **kill switch** y alerta. La regla para reconocer una cuenta paper (el prefijo `DU` u otra) **queda a confirmar** en la documentación oficial; hasta entonces se compara con el ID de cuenta paper de `.env`, y si no coincide no se conecta.
3. **Etapa 1 en solo lectura:** el modo solo lectura de la API queda activado en IB Gateway. El adaptador de la etapa 1 no tiene código para enviar órdenes.
4. **Riesgo:** toda orden pasa por `cfds_risk::check`, como en el `PaperBroker`. No hay otro camino para obtener un tamaño.
5. **Órdenes idempotentes:** cada orden lleva nuestro `order_id` (`ClientId`), que se guarda junto al ID de IBKR. Reenviar el mismo `order_id` devuelve el informe de la primera y nunca abre dos posiciones.
6. **Kill switch:** se activa ante límite superado, error de datos, desconexión de IB Gateway o una cuenta que no es paper. No se desactiva desde el adaptador (spec §1.2).

### 3. Dónde vive

- **En `engine/` (Rust)**, en un crate nuevo `cfds-ibkr` (`engine/crates/ibkr`), igual que `cfds-ig`. Implementa `MarketDataProvider` (precios) y el trait `Broker` de `cfds-broker` (cuenta paper de IBKR). La fila del registro de módulos se agrega en el PR que lo cree.
- **Librería:** la candidata es el crate [`ibapi`](https://crates.io/crates/ibapi) ([repositorio](https://github.com/wboayue/rust-ibapi)): licencia **MIT**, versión 5.7.0 del 06-11-2026, actividad en el repositorio el 23-11-2026, unas 316.000 descargas, pide Rust 1.88. **No es oficial de IBKR**: es de la comunidad. Otros crates (`ibkr`, `twsapi`) tienen poco uso o están abandonados (2021). Antes de usar `ibapi` hay que revisar su código de conexión y fijar la versión exacta.
- **Si `ibapi` no sirve:** implementar en Rust solo la parte del protocolo de la TWS API que necesitamos (conexión, cuentas, contratos, precios, órdenes). La alternativa de usar la librería oficial de Python queda descartada (ver abajo).
- **Los cálculos siguen en Rust:** costos, P/L, margen y riesgo usan `cfds-backtest` y `cfds-risk`, con `rust_decimal`. Los precios de IBKR se convierten a `Decimal` al entrar; nunca se usa `float` para dinero.
- **Contratos:** lo que cruce entre lenguajes reusa `order.v1`, `order_report.v1`, `position.v1` y `paper_account.v1`. Si hace falta algo nuevo (por ejemplo, el origen `ibkr_paper` de un informe o las velas de un futuro de IBKR), se define en `contracts/` en el mismo PR o en uno previo, nunca inventado.

### 4. Plan por etapas

Cada etapa es su propio Issue y su propio PR, y empieza solo con el OK del dueño.

1. **Solo lectura.** Conectar a la cuenta paper, leer el estado de la cuenta y los precios de MNQ (y MES). Guardar velas cerradas con su fuente. Verificar las guardas (tests: puerto real rechazado, cuenta no paper rechazada, desconexión activa el kill switch).
2. **Órdenes paper de MNQ con riesgo.** Envío por el trait `Broker`, con `cfds_risk::check`, stop obligatorio, IDs propios e idempotencia. Conciliación entre lo que creemos tener y lo que informa IBKR (`reconcile`).
3. **Comparar costos, IG demo frente a IBKR paper, durante N semanas** (N lo fija el dueño antes de empezar; propuesta: 4). Misma señal y mismo riesgo en los dos. Se mide spread o tick efectivo, comisión, tasas, slippage y costo de mantener la posición. Agrega su fila al registro de resultados. Ojo: el llenado de una cuenta paper es **simulado por IBKR** y puede no reflejar el slippage real.

## Lo que tiene que confirmar el dueño

1. **Permiso de futuros:** hoy figura **denegado** (#493). ¿Se vuelve a pedir? Sin él, la cuenta paper no puede operar futuros (hereda los permisos de la real).
2. **Tipo de cuenta:** si alcanza la cuenta **Cash** para futuros o hace falta pasar a **Margin** (leerlo en la FAQ oficial o preguntarlo a IBKR, consulta 3 de `docs/guides/consultas-proveedores.md`).
3. **Depósito:** la cuenta paper no se abre sin fondos en la real. ¿Cuánto y cuándo? Es un gasto del dueño: ningún agente lo hace ni lo pide por su cuenta.
4. **Datos de CME:** costo mensual actual para no profesional (en el gestor de suscripciones), si hay exención por comisiones y cómo se comparte con la cuenta paper.
5. **Puertos de IB Gateway** (paper y real) y el **prefijo o el ID de la cuenta paper**, leídos en la configuración de IB Gateway y en la documentación oficial.
6. **Tasas de bolsa y regulatorias** de MNQ y MES (IBKR y CME), y la **especificación** de los dos contratos en CME.
7. **N semanas** de la etapa 3.
8. **Dónde corre IB Gateway:** el Mac mini, con el login hecho por el dueño cada vez que IB Gateway lo pida.

## Alternativas consideradas

- **Seguir solo con IG:** no permite medir el costo de un futuro; la tasa administrativa del CFD sigue en todo lo que se mantiene varios días.
- **Client Portal Web API (REST) en vez de la TWS API:** también necesita un gateway local con login del dueño, y el crate de Rust más usado es para la TWS API. Se puede reconsiderar si la TWS API da problemas.
- **Librería oficial de Python (`ibapi` de IBKR) en `intelligence/`:** rompe la regla de que órdenes, riesgo y P/L viven solo en Rust (`CLAUDE.md` §1). Descartada.
- **CFD de índices de IBKR:** no aportan nada frente a IG para este objetivo (el motivo es dejar la tasa administrativa). Además no se pudo leer si se ofrecen a minoristas en España (consulta 3).
- **Otro broker de futuros:** el dueño ya tiene la cuenta de IBKR y sus comisiones de micro están leídas. Cambiar de broker pediría otro ADR.
- **Dinero real con IBKR:** **bloqueado**. Requiere un ADR aprobado por dos personas (`CLAUDE.md` §5, fase 11); mientras el dueño construya solo, no se puede.

## Riesgos

- **Confundir la sesión real con la paper:** es el riesgo principal, porque la diferencia es el login y el puerto. Mitigación: las guardas de §2 (puerto, cuenta, kill switch) con tests, más el modo solo lectura en la etapa 1.
- **Crate de la comunidad:** `ibapi` puede cambiar su interfaz o dejar de mantenerse. Mitigación: versión fija, adaptador detrás de los traits del proyecto, y la alternativa de implementar el protocolo en Rust.
- **Llenados simulados:** la cuenta paper de IBKR no es el mercado real; sus llenados pueden ser más optimistas. La comparación de la etapa 3 lo dice en su informe.
- **Costos que cambian:** comisiones y datos se vuelven a leer antes de cada etapa, con su fecha.
- **Login manual:** si IB Gateway pide volver a entrar, el adaptador queda desconectado y el kill switch se activa; no hay reconexión con credenciales guardadas.
- **Gastos:** suscripción de datos y depósito, solo con el OK del dueño.

## Consecuencias

- Aparece un segundo broker detrás de los mismos traits (`MarketDataProvider`, `Broker`): la estrategia y el riesgo no cambian.
- Hay un camino para medir con precios reales cuánto cuesta un futuro micro frente al CFD, sin dinero real.
- Agrega mantenimiento: IB Gateway en el Mac mini, un crate externo y el login manual del dueño.
- Mientras falten los datos de "Lo que tiene que confirmar el dueño", ninguna etapa usa esos valores como si fueran ciertos.
