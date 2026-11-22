# 0007 – X (ex Twitter) como fuente de sentimiento

- **Estado:** propuesta
- **Fecha:** 2026-11-22
- **Aprobada por:** pendiente (las dos personas)
- **Issue:** #135 (relacionado: ADR 0004 y #26, #25, #99)

## Contexto

El dueño propuso leer lo que publican en X **las cuentas que elijamos** (analistas, bancos centrales, medios, personas influyentes en el mercado) y usarlo como fuente de sentimiento.

El ADR 0004 había dejado las redes sociales **fuera de v1** (#26). Este ADR reabre el tema con la información verificada, para que lo decidan las dos personas. Como toda fuente nueva, necesita una vía de acceso legal y documentada (spec §9, ADR 0003 §C.4).

Todo lo que sigue se leyó en las **páginas oficiales de X** el 22-11-2026. `developer.x.com` respondió "402 Payment Required" a los pedidos automáticos, así que se usó `docs.x.com`, que publica los mismos documentos. Lo que no se pudo verificar queda **pendiente**.

## Lo verificado

### Acceso y costo

| Punto | Dato | Fuente |
|---|---|---|
| Única vía | API oficial de X. **No se hace scraping:** el proyecto no esquiva protecciones ni términos (ADR 0004). Los Términos de Servicio de X (`x.com/en/tos`) respondieron 402 y no se pudieron leer de forma automática; si hiciera falta citarlos, los lee una persona. | [Agreement](https://docs.x.com/developer-terms/agreement.md) |
| Modelo de cobro | "The X API uses pay-per-usage pricing. No subscriptions—pay only for what you use." Créditos que se compran antes. **No hay un plan gratuito** descrito en la documentación. | [Pricing](https://docs.x.com/x-api/getting-started/pricing.md) |
| Precio de lectura | Posts: "$0.005 per resource". Usuarios: "$0.010 per resource". | Ídem |
| Tope | "Pay-per-usage plans are capped at 3 million Post reads per monthly billing cycle". Se puede fijar un límite de gasto: al llegar, "API requests will be blocked until the next billing cycle". | Ídem |
| Promoción | "$20 in free X API credits" al cargar la primera tarjeta y hacer la primera compra. No es un plan gratuito. | Ídem |
| Límites por endpoint | Publicaciones de una cuenta (`GET /2/users/:id/tweets`): 10.000 pedidos cada 15 min por app. Búsqueda de los últimos 7 días: 450 cada 15 min. Búsqueda en todo el archivo: 1 por segundo. | [Rate limits](https://docs.x.com/x-api/fundamentals/rate-limits) |
| Historia | Búsqueda de los últimos 7 días para todos. **Archivo completo desde marzo de 2006**: "available to pay-per-use and Enterprise customers". | [Search](https://docs.x.com/x-api/posts/search/introduction.md) |
| En vivo | *Filtered stream* con pago por uso: 1 conexión y hasta 1.000 reglas. | [Filtered stream](https://docs.x.com/x-api/posts/filtered-stream/introduction.md) |
| Alta | Cuenta de desarrollador con nombre, descripción y **caso de uso**, y aceptar el Developer Agreement. | [Getting access](https://docs.x.com/x-api/getting-started/getting-access.md) |

### Términos de uso

| Punto | Dato | Fuente |
|---|---|---|
| Guardar posts | Permitido: "Copy a reasonable amount of and display the X Content" (II.A.2). | [Agreement](https://docs.x.com/developer-terms/agreement.md) |
| Mantenerlos al día | Si un post se borra, se vuelve privado o se suspende, hay que borrarlo o modificarlo "within twenty four (24) hours" de un pedido escrito de X o del usuario (IV.B). La Policy pide mantener "up to date" lo guardado. | Agreement y [Policy](https://docs.x.com/developer-terms/agreement-and-policy.md) |
| Análisis | Permitido "conduct analysis of the X Content, as explicitly approved by X" (II.A.1), nunca de forma "inconsistent with X users' reasonable expectations of privacy" (XIV.B). | Agreement |
| Modelos de IA | Prohibido "fine-tune or train a foundation or frontier model" (III.A(k)). Un modelo propio y chico de sentimiento no es un modelo fundacional, pero los términos no lo dicen de forma explícita: **pendiente de confirmar con X**. | Agreement |
| Datos sensibles | Prohibido inferir o guardar información inferida sobre un usuario, como su "negative financial status" o su afiliación política, y hacer "individual profiling". | [Restricted use cases](https://docs.x.com/developer-terms/restricted-use-cases.md) |
| Uso comercial | Un producto comercial o monetizado requiere el plan Enterprise (III.M). Si un sistema **privado** de un grupo, sin vender nada, cuenta como comercial **no está claro**: **pendiente de confirmar con X**. | Agreement |
| Trading y análisis de mercado | **No se mencionan** en ninguna de las páginas leídas, ni para permitirlos ni para prohibirlos. | — |

## Cuánto costaría (estimación, precios del 22-11-2026)

Con el precio de 0,005 USD por post leído:

| Uso | Supuesto | Costo |
|---|---|---|
| En vivo | 30 cuentas × 10 posts por día ≈ 9.000 posts por mes | ≈ **45 USD por mes** |
| Historia para el backtest | Esas 30 cuentas, 3 años ≈ 330.000 posts | ≈ **1.650 USD**, una sola vez |
| Historia mínima | 10 cuentas, 1 año ≈ 36.500 posts | ≈ **180 USD**, una sola vez |

Los volúmenes por cuenta son supuestos: se miden con la primera carga, con un límite de gasto puesto en la consola de X.

## Decisión (propuesta)

**No incorporar X por ahora.** Motivos:

1. **No es gratis.** El dueño pidió fuentes sin costo (#135 y `docs/research/fuentes-datos-gratis-2026-11.md`), y X cobra cada post leído. Para el backtest hace falta historia, que es la parte cara.
2. **Hay dos dudas de términos que solo X puede responder:** si un sistema privado de un grupo es "uso comercial" (y entonces requiere Enterprise), y si un modelo de sentimiento propio entra en la prohibición de entrenar modelos.
3. **Parte de lo que se busca ya lo tenemos gratis:** las cuentas de la Fed y del BCE repiten sus comunicados, que ya se leen por RSS (ADR 0006).
4. **Que aporte se puede medir antes de pagar:** con el sentimiento que ya se graba (clientes de IG, COT y medidas propias, ADR 0004 §7), la fase 5 dice si el sentimiento mejora fuera de muestra. Si mejora, X se reevalúa con datos.

**Si las dos personas deciden incorporarlo igual**, se propone:
- Pago por uso, con **límite de gasto** en la consola de X y empezando sin historia: solo en vivo.
- **Lista de cuentas en `config/`** (nunca en el código), solo cuentas públicas de instituciones, medios y analistas.
- **No guardar el texto:** solo el ID del post, la cuenta, la hora de publicación y el puntaje derivado sobre el **mercado**, nunca sobre la persona. Así se reduce la obligación de borrar en 24 h, que igual se cumple con el ID, y no se guardan datos personales de más (RGPD).
- Cada post vale **desde su hora de publicación** (CLAUDE.md §5b).
- Antes de empezar, preguntarle a X las dos dudas del punto 2.

## Alternativas consideradas

- **Scraping de X:** descartado. El proyecto solo usa vías de acceso documentadas (ADR 0004), y X ofrece una API oficial.
- **Plan Enterprise:** precio no publicado (formulario de contacto). Solo tiene sentido si X dice que nuestro uso es comercial.
- **Otras redes sociales (Reddit, StockTwits):** no evaluadas. Cada una necesita su propia revisión de términos y costo.

## Consecuencias

- Si se acepta esta propuesta, X queda fuera hasta que la fase 5 muestre que el sentimiento aporta. El ADR 0004 sigue vigente.
- Si se decide incorporarlo, hace falta un Issue de implementación: contrato, ingesta en `intelligence/`, configuración de cuentas y el proceso de borrado en 24 h. Además, pagar los créditos y responder las dudas con X.
- Los precios y términos de X cambian sin aviso: antes de cualquier implementación, **volver a verificarlos**.
