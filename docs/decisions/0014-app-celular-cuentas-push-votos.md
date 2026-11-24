# 0014 – App del celular: cuentas con 2FA, notificaciones push, votaciones y acceso desde fuera de casa

- **Estado:** propuesta
- **Fecha:** 2026-11-24
- **Aprobada por:** pendiente. Mientras el dueño construya solo, la aprueba el dueño (`CLAUDE.md`, modo de un solo constructor). Nada de este ADR habilita dinero real ni órdenes reales.
- **Issue:** #453 (sigue a #419: fase 10 aprobada para avanzar, con el criterio pendiente de cuentas con 2FA, push, votaciones y acceso desde el celular, #275)
- **Relacionados:** spec de la fase 1 §1, §1.1 y §1.2; `docs/ROADMAP.md`, fase 10; `contracts/change_proposal.v1`, `change_vote.v1` y `change_vote_status.v1` (#327, #331); `config/votes.toml`; PWA instalable (#307, PR #309); alertas por Telegram de la fase 9 (`cfds-obs`, #297)

## Contexto

La fase 10 pide una **app privada para los iPhone de los 6 miembros** con notificaciones push, aprobación de propuestas, votaciones y cuentas individuales con 2FA (ROADMAP, fase 10; spec §1.1). Son decisiones de seguridad: este ADR elige cómo, y **no implementa nada**. Tampoco se creó ninguna cuenta, clave ni credencial.

### Lo que ya existe (24-11-2026, `origin/main`)

- **PWA (#307, PR #309).** `ui/public/manifest.webmanifest` con `display: "standalone"`, íconos y `apple-touch-icon`. `ui/public/sw.js` guarda solo el armazón y **nunca** `/api`. No tiene manejador de `push` ni de `notificationclick`.
- **Ingreso de maqueta.** `ui/src/components/ui/sign-in.tsx` pide email, contraseña y un código TOTP de 6 dígitos, pero "no envía nada a ningún lado".
- **Gateway.** `engine/crates/gateway` escucha en `127.0.0.1:8787` (`config/gateway.toml`). Casi todo es de solo lectura. Los `POST` del broker simulado exigen el token local anti-CSRF `x-cfds-token`, que agrega el proxy de Vite del lado del servidor (#292). **No hay rutas para votar** (`GET /api/v1/learning_report/votes` solo lee el conteo).
- **Votos.** Tablas `change_proposals` y `change_votes` de solo agregar (`infra/migrations/0034_learning_votes.sql`), con un voto por miembro y propuesta (`UNIQUE (change_id, member_id)`). El conteo es `cfds_journal::votes::tally` con `config/votes.toml` (`members = 6`, `majority = 4`, `window_hours = 48`). Contar votos nunca aplica un cambio.
- **Módulo `auth` planeado** en `docs/ARCHITECTURE.md`: "Cuentas de los miembros, 2FA (TOTP), roles (responsable / miembro), sesiones, registro de acciones".
- **Alertas.** `cfds-obs` ya manda alertas por log y por Telegram (kill switch, error de datos, desconexión, racha de rechazos).

### Lo que se comprobó en fuentes oficiales (24-11-2026)

**Web Push en iOS** ([WebKit: Web Push for Web Apps on iOS and iPadOS](https://webkit.org/blog/13878/web-push-for-web-apps-on-ios-and-ipados/); [Apple: Sending web push notifications in web apps and browsers](https://developer.apple.com/documentation/usernotifications/sending-web-push-notifications-in-web-apps-and-browsers)):

- Funciona en **iOS/iPadOS 16.4 o posterior**, solo en la web app **agregada a la pantalla de inicio**, con un manifest de `display` `standalone` o `fullscreen`. La PWA de #307 ya lo cumple.
- Usa los estándares Push API, Notifications API y Service Workers, con claves **VAPID** que identifican al servidor ([RFC 8292](https://www.rfc-editor.org/rfc/rfc8292)) y el contenido cifrado ([RFC 8291](https://www.rfc-editor.org/rfc/rfc8291), sobre [RFC 8030](https://www.rfc-editor.org/rfc/rfc8030)).
- **No hace falta el Apple Developer Program.** El servidor tiene que poder salir a `https://*.push.apple.com`.
- El permiso se pide **con un gesto del usuario** (tocar un botón), y la suscripción se llama dentro de ese mismo gesto.
- **No hay push invisibles:** cada push tiene que mostrar una notificación enseguida. Si no, Safari revoca el permiso.
- Contenido de hasta **4 KB**. Cabeceras `TTL` (hasta 30 días) y `Urgency` (`high` para intentar entrega inmediata). El JWT de VAPID dura como máximo un día, y no se renueva más de una vez por hora. Un `410` significa que la suscripción ya no sirve.
- Sirve el contador en el ícono (Badging API: `setAppBadge` y `clearAppBadge`).
- Desde iOS 26, todo sitio que se agrega a la pantalla de inicio abre como web app por defecto ([WebKit: Safari 26.0](https://webkit.org/blog/17333/webkit-features-in-safari-26-0/)).
- Según Apple, la web app de la pantalla de inicio tiene **cookies y almacenamiento separados de Safari** ([WWDC23: What's new in web apps](https://developer.apple.com/videos/play/wwdc2023/10120/)). El ingreso se hace dentro de la app instalada, no en Safari.

**Segundo factor:**

- TOTP es el estándar [RFC 6238](https://www.rfc-editor.org/rfc/rfc6238). Es lo que fija la spec §1.1 ("app autenticadora, Google Authenticator o similar").
- Las passkeys son credenciales WebAuthn ([W3C WebAuthn nivel 3](https://www.w3.org/TR/webauthn-3/)), atadas al dominio (RP ID) y resistentes al phishing. NIST las trata como autenticadores resistentes al phishing; TOTP no lo es ([NIST SP 800-63B](https://pages.nist.gov/800-63-4/sp800-63b.html)).
- **No se encontró una fuente oficial** de Apple o WebKit sobre passkeys dentro de una web app de la pantalla de inicio. Hay reportes de desarrolladores, sin confirmar, de fallas en ese caso.

**Acceso remoto:**

- **Tailscale Serve** publica un servicio local con HTTPS solo dentro de la red privada (tailnet), con un nombre `equipo.tailnet.ts.net` y un certificado de Let's Encrypt ([Tailscale Serve](https://tailscale.com/kb/1312/serve); [HTTPS en Tailscale](https://tailscale.com/kb/1153/enabling-https)).
  - El nombre del equipo queda publicado en los registros de transparencia de certificados. Tailscale advierte que no se use un nombre con información sensible.
  - **Funnel**, la variante pública, viene habilitada por defecto y hay que apagarla.
  - Plan **Personal gratis**: hasta **6 usuarios** y dispositivos ilimitados, **solo para uso no comercial** ([precios de Tailscale](https://tailscale.com/pricing)).
- **Cloudflare Tunnel** conecta solo hacia afuera, con `cloudflared` y sin puertos abiertos ([Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/)). Cloudflare Zero Trust es gratis hasta **50 usuarios** ([planes de Cloudflare Zero Trust](https://www.cloudflare.com/plans/sase-zero-trust/)).
- **Unión Europea:** en 2024 Apple anunció que sacaba las web apps de la pantalla de inicio en la UE y después dio marcha atrás con iOS 17.4 ([Apple Developer, actualización sobre apps en la UE](https://developer.apple.com/jp/support/dma-and-apps-in-the-eu/)). Hoy funcionan, pero depende de Apple. La versión en inglés de esa página ya no trae el texto: queda **a verificar**.

### Lo que no se pudo confirmar (no se inventa)

- Si las passkeys funcionan bien dentro de la web app instalada en los iPhone de los miembros. Se prueba antes de adoptarlas (ver Decisión, punto 1).
- El precio de los planes pagos de Tailscale, que haría falta con un séptimo usuario.
- Cómo se comporta el inicio de sesión de Cloudflare Access dentro de una PWA instalada (redirecciones y cookies del service worker).

## Decisión

Se propone lo siguiente. Cada punto lo aprueba o lo cambia el dueño.

### 1. Autenticación: cuentas propias en PostgreSQL, contraseña + TOTP obligatorio (passkeys después)

- **Dónde vive el usuario:** en nuestra base, módulo Rust `auth` (`engine/crates/auth`, ya planeado). Tablas nuevas `members` y `member_sessions`. Sin proveedor externo de identidad.
- **Método en v1, tal como dice la spec §1.1:**
  - Email + contraseña guardada con **Argon2id**.
  - **TOTP obligatorio** (RFC 6238): 6 dígitos, paso de 30 s, ±1 paso de tolerancia.
  - Un código ya usado no se acepta de nuevo: se guarda el último paso aceptado.
  - El secreto TOTP se guarda cifrado, con la clave en `.env`, nunca en el repo.
  - 10 **códigos de recuperación** de un solo uso, guardados con hash.
- **Alta de miembros:**
  - El dueño crea una invitación de un solo uso y con vencimiento, con un comando local en el Mac mini.
  - El miembro la abre **desde la app instalada**, elige su contraseña y escanea el QR del TOTP.
  - El rol (`responsable` o `miembro`) lo fija el dueño. No hay registro abierto.
- **Sesiones:**
  - Token opaco aleatorio de 256 bits, guardado con hash en `member_sessions`.
  - Cookie `__Host-` con `HttpOnly`, `Secure`, `SameSite=Strict` y `Path=/`.
  - Vencimiento por inactividad y absoluto en `config/` (propuesta inicial: 14 y 30 días; en un celular, pedir el código todos los días cansa).
  - Cierre de sesión y revocación de todas las sesiones de un miembro.
  - Guía: [OWASP Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html).
- **Segundo paso para acciones sensibles:** votar, aprobar o rechazar una propuesta piden un **código TOTP fresco** (o uno aceptado en los últimos 5 minutos, valor en `config/`). Una sesión robada sola no alcanza para votar.
- **Límite de intentos:** bloqueo temporal después de 5 fallos seguidos de contraseña o TOTP (valores en `config/`), con alerta.
- **Passkeys, en una segunda etapa:**
  - Se suman cuando se haya probado que funcionan en la web app instalada de los iPhone de los miembros.
  - Primero como reemplazo de la contraseña, con TOTP o la propia passkey con biometría como segundo factor.
  - Cambia la spec §1.1, así que necesita su propia aprobación.
  - El RP ID es el nombre `.ts.net` del punto 4: **ese nombre se fija antes** y no se cambia, porque cambiarlo invalida las passkeys y las suscripciones push.

### 2. Notificaciones push: Web Push estándar con VAPID, enviado por el motor

- **Quién envía:** el motor (Rust), como un destino más de `cfds-obs` (log, Telegram y **push**). El gateway guarda las suscripciones.
  - El servidor solo **sale** hacia `*.push.apple.com`. No abre puertos.
  - Las claves VAPID son un secreto: van en `.env` y se documentan en `.env.example` con valores falsos.
- **En la app:**
  - Un botón "Activar notificaciones" (el gesto que pide Apple) se suscribe con la clave pública VAPID y manda la suscripción al gateway.
  - `sw.js` suma los manejadores `push` y `notificationclick`, que muestran **siempre** la notificación, porque no hay push invisibles.
  - El manejador de `fetch` sigue sin guardar `/api`.
- **Qué se notifica (v1):**

| Evento | Urgency | TTL | Texto en la pantalla bloqueada |
|---|---|---|---|
| Nueva propuesta (para los responsables) | `high` | la validez de la propuesta (spec: 10 min) | "Nueva propuesta en simulación: US500" (sin montos) |
| Kill switch activado | `high` | 24 h | "Kill switch activado (simulación). Abrí la app." |
| Error de datos o desconexión (las alertas de `cfds-obs`, con su límite de repetición) | `high` | 1 h | tipo de alerta, sin detalles internos |
| Votación abierta | `normal` | hasta el plazo (48 h) | "Hay una votación abierta" |
| Votación por vencer (6 h antes, a quien no votó) | `normal` | hasta el plazo | "Quedan 6 h para votar" |
| Resultado de una votación | `low` | 7 días | "Votación cerrada: aprobada o rechazada" |

- **Reglas del contenido:**
  - Nunca montos ni P&L en la pantalla bloqueada.
  - Siempre "simulación" mientras no haya dinero real.
  - Ningún texto promete ganancias (`CLAUDE.md` §5).
  - El detalle se ve recién al abrir la app, ya con sesión.
- **Push no es garantía de entrega.** Ninguna regla depende de que llegue un push:
  - el plazo de una votación corre igual;
  - una propuesta vence igual;
  - Telegram sigue como canal de respaldo de las alertas.
- **Suscripciones:** una por dispositivo y miembro. Se borran ante un `410`. Al cerrar sesión, se da de baja la del dispositivo.
- **Contratos primero:**
  - `push_subscription.v1`: endpoint, claves `p256dh` y `auth`, `member_id`, `created_at`.
  - `push_message.v1`: tipo de evento, título, cuerpo, URL interna, `urgency`, `ttl`, `topic` y `created_at`.
  - Ambos van en `contracts/` antes del código.

### 3. Votaciones desde la app (§1.2)

- **Quién vota:** cada miembro con cuenta activa, **un voto por miembro**, ya forzado por `UNIQUE (change_id, member_id)`.
  - El `member_id` del voto **lo pone el servidor desde la sesión**, nunca el cuerpo del pedido.
  - El voto no se cambia: las tablas son de solo agregar, y la spec no prevé cambiar un voto.
- **Ruta nueva:** `POST /api/v1/change_votes`.
  - Con sesión, código TOTP fresco (punto 1), `application/json` y `vote_id` idempotente.
  - Escribe un `change_vote.v1` con `voted_at` = hora del servidor en UTC. El conteo sigue siendo `cfds_journal::votes::tally`, sin lógica en la UI.
- **Quórum y plazos:** los de `config/votes.toml` y la spec §1.2. Hoy solo existe la fila de "cambios de reglas o parámetros" (4 de 6 en 48 h; si no se llega, queda rechazada).
- **Qué entra en v1:** solo las propuestas de cambio del informe de aprendizaje (`change_proposal.v1`). **Contar votos nunca aplica el cambio:** el parámetro se cambia a mano en `config/` con el voto registrado.
- **Qué queda afuera de v1:**
  - **Reactivar después del kill switch** (4 de 6 y los 2 responsables): sigue manual, hasta que se defina quiénes son los responsables mientras el dueño construye solo.
  - **Operación grande** (4 de 6 en 10 min).
  - **Pasar a dinero real** (6 de 6): **la app no lo ofrece nunca**. La fase 11 está bloqueada y pide un ADR de dos personas (`CLAUDE.md` §5).
- **Auditoría:**
  - Tabla nueva `member_actions`, de solo agregar con el mismo trigger que `learning_append_only`, y contrato `member_action.v1`.
  - Registra miembro, acción (ingreso, ingreso fallido, cierre de sesión, voto, aprobar o rechazar propuesta, alta o baja de push, reseteo de TOTP), objeto, hora UTC e id de sesión con hash.
  - Cumple la spec §1.1: "toda acción queda registrada con usuario y hora (UTC)".
  - Los logs de `cfds-obs` nunca escriben contraseñas, códigos, secretos TOTP ni endpoints de push.

### 4. Acceso desde fuera de casa: Tailscale (red privada), sin nada público

- El Mac mini y los iPhone de los miembros entran a una **tailnet** de Tailscale.
- La app se sirve con **`tailscale serve`** en `https://<nombre-neutro>.<tailnet>.ts.net`, con certificado válido. Esa es la URL que cada miembro agrega a la pantalla de inicio, y el origen de la PWA, del push y, más adelante, de las passkeys.
- El gateway sigue escuchando solo en `127.0.0.1`.
- **Funnel apagado.** Las reglas de acceso (ACL) dejan entrar a los miembros solo al puerto HTTPS del Mac mini.
- Para **abrir** la app hace falta Tailscale conectado. **Los push llegan igual**, porque vienen de Apple.
- La seguridad queda en dos capas: estar en la tailnet **y** la cuenta con TOTP. La red nunca es el único control.
- **Costo:** 0 € con el plan Personal (hasta 6 usuarios). Apple Web Push: 0 €. Apps autenticadoras: gratis.

## Alternativas consideradas

### Autenticación

| Opción | A favor | En contra |
|---|---|---|
| **A. Contraseña + TOTP propios (elegida para v1)** | Es lo que pide la spec §1.1. El dato queda en casa. Funciona en cualquier iPhone. Ya hay una maqueta de ingreso que lo pide. | TOTP no es resistente al phishing (riesgo bajo detrás de la tailnet). Hay que implementar bien el hash, el cifrado del secreto, el límite de intentos y la recuperación. |
| B. Passkeys (WebAuthn) como método principal ya | Resistente al phishing. Sin contraseña. Se sincroniza con el llavero de iCloud. | Sin confirmación oficial dentro de la web app instalada, y hay reportes de fallas. Cambia la spec. Atada al dominio: si cambia el nombre `.ts.net`, hay que registrarlas de nuevo. Queda como segunda etapa. |
| C. Proveedor externo (Auth0, Supabase Auth, Cloudflare Access como único login) | Menos código propio. | Los datos de los miembros salen de casa. Dependencia y posibles costos. Igual hace falta el `member_id` propio para votos y auditoría. Más piezas en la PWA instalada (redirecciones). |
| D. Solo la identidad de la red (estar en la tailnet) | Cero código de cuentas. | No cumple "cuenta individual con 2FA" de la spec. Un celular desbloqueado con Tailscale alcanza para votar. La red sería el único control. |

### Notificaciones

| Opción | A favor | En contra |
|---|---|---|
| **A. Web Push estándar con VAPID desde el motor (elegida)** | Gratis, sin cuenta de Apple. Estándar (RFC 8030, 8291 y 8292). Encaja con la PWA de #307. El contenido viaja cifrado. | Requiere iOS 16.4 o posterior y la app en la pantalla de inicio. No hay push invisibles. La entrega no está garantizada. Depende de la política de Apple sobre web apps (antecedente de la UE en 2024). |
| B. Solo Telegram (ya existe) | Ya funciona en la fase 9. | No hay cuenta por miembro ni 2FA, y no sirve para votar con auditoría. Los mensajes pasan por un tercero sin cifrado de extremo a extremo en los chats comunes. Se queda como respaldo. |
| C. Servicio de push de terceros (por ejemplo OneSignal) | Panel y reintentos ya hechos. | Un tercero más con los datos y los dispositivos de los miembros, para algo que el estándar ya resuelve. |
| D. App nativa | Push y passkeys más confiables. | Apple Developer Program pago, distribución (TestFlight o similar) y otro lenguaje. La spec §1.1 lo deja para "si más adelante hace falta". |

### Acceso desde fuera de la LAN

| Opción | A favor | En contra |
|---|---|---|
| **A. Tailscale + `tailscale serve` (elegida)** | Nada expuesto a Internet. HTTPS válido sin dominio propio. Gratis hasta 6 usuarios. Instalación simple en el iPhone. | **Justo 6 usuarios:** un séptimo obliga a un plan pago, con precio no verificado. El plan Personal es **solo para uso no comercial**: un fondo del grupo puede no entrar, y **el dueño tiene que leer los términos**. Cada miembro necesita una cuenta de un proveedor de identidad para Tailscale y tener la app conectada. El nombre del equipo queda publicado en los registros de certificados. |
| B. Cloudflare Tunnel + Access | Sin puertos abiertos. Gratis hasta 50 usuarios. No hace falta app en el celular. | Hace falta un dominio propio (costo anual). Cloudflare termina el TLS y ve el tráfico en claro. El nombre es público en Internet, protegido por Access. Comportamiento de Access dentro de la PWA instalada sin verificar. |
| C. WireGuard propio con puerto abierto en el router | Sin terceros. | Puerto abierto, IP pública o DNS dinámico, y un certificado HTTPS que igual pide dominio. Más mantenimiento para una sola persona. |
| D. Nada: solo dentro de la casa | Lo más simple y cerrado. | Los otros 5 miembros no viven en esa red: **no podrían votar ni aprobar**, y no se cumple la fase 10. HTTPS igual hace falta para el service worker y el push. |
| E. Abrir el puerto del gateway a Internet, con solo el login | — | Descartada: deja el motor expuesto a todo Internet y depende de que el login no tenga ni un error. |

## Consecuencias

- **Lo que habilita:** cumplir el criterio pendiente de la fase 10 (#275) con costo recurrente de 0 € y sin exponer nada a Internet.
- **Lo que hay que construir después.** Cada punto es su Issue y su PR, en este orden, y nada se hace en este PR:
  1. Contratos: `member.v1` (sin secretos), `member_action.v1`, `push_subscription.v1` y `push_message.v1`.
  2. Migraciones: `members`, `member_sessions`, `member_invites`, `member_actions` (solo agregar) y `push_subscriptions`.
  3. Crate `auth`: Argon2id, TOTP, sesiones, invitaciones, límite de intentos y segundo paso para acciones sensibles. Las librerías se eligen en su Issue: tienen que cumplir RFC 6238 y estar mantenidas.
  4. Gateway:
     - rutas de ingreso, cierre de sesión, suscripción push y `POST /api/v1/change_votes`;
     - el token `x-cfds-token` del proxy de Vite se reemplaza por la sesión para la app;
     - un servidor de producción para el build de la UI, en lugar del proxy de desarrollo de Vite.
  5. Envío de Web Push (VAPID, RFC 8291 y 8292) como destino de `cfds-obs`.
  6. UI: ingreso real en `sign-in.tsx`, alta con QR del TOTP, botón de notificaciones, manejadores `push` y `notificationclick` en `sw.js`, y votar desde la tarjeta de aprendizaje (que hoy dice "sin votar desde la UI").
  7. Guía en `docs/guides/` para que cada miembro instale Tailscale y la app, active el TOTP y las notificaciones.
- **Lo que hace el dueño, no un agente:**
  - crear la tailnet y las cuentas de Tailscale;
  - aceptar los términos;
  - elegir el nombre neutro del Mac mini antes de pedir el certificado;
  - generar las claves VAPID y la clave de cifrado del TOTP en el `.env` del Mac mini.
  - Ningún agente crea cuentas ni credenciales (`CLAUDE.md` §5).
- **Se vuelve más difícil:**
  - todo depende del Mac mini encendido: si se apaga, no hay app ni push (Telegram tampoco);
  - un miembro que pierde el celular necesita sus códigos de recuperación, o que el dueño le resetee el TOTP (queda auditado);
  - sumar un séptimo miembro obliga a revisar el plan de Tailscale y `config/votes.toml`.
- **Riesgos:**
  - Apple puede cambiar el soporte de web apps; el antecedente es la UE en 2024.
  - El push puede no llegar (modos de concentración, permiso revocado, celular apagado): por eso ninguna regla depende de él.
  - Un error en el código propio de cuentas: se mitiga con tests de casos borde (código reusado, sesión vencida, voto duplicado, voto fuera de plazo, bloqueo por intentos) y con la tailnet como primera capa.
- **Decisiones que el dueño tiene que tomar al aprobar:**
  1. Tailscale (A) o Cloudflare (B) para el acceso, después de leer los términos de uso no comercial del plan Personal.
  2. Contraseña + TOTP ahora y passkeys después, o esperar a passkeys.
  3. La tabla de qué se notifica y sus textos.
  4. Vencimientos de sesión (14 y 30 días) y ventana del segundo paso (5 min).
  5. Quiénes son los "responsables" mientras construye solo, para aprobar propuestas y, más adelante, para reactivar después del kill switch.
