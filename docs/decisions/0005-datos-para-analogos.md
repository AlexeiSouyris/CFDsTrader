# 0005 – Datos para análogos: componentes históricos del NASDAQ 100 y precios

- **Estado:** aceptada
- **Fecha:** 2026-11-20 (actualizada el 21-11-2026: análogos v1 con diarias de los índices de IG)
- **Aprobada por:** las dos personas, el 21-11-2026 (informado por el dueño)
- **Issue:** #10

> El número 0004 queda reservado para las fuentes de sentimiento (#26).

## Contexto

La spec §13 pide análogos sobre un universo de **índices + componentes del NASDAQ 100, 5 años, incluidas las empresas que salieron del índice** (sin sesgo de supervivencia). Hacen falta dos cosas:
1. **Qué empresas estaban en el índice en cada fecha.**
2. **Precios de esas empresas** (incluidas las que ya no cotizan), ajustados por splits y dividendos.

El grupo decidió empezar con fuentes **gratuitas** (spec §2).

## 1. Componentes históricos: decidido (propuesta)

**[nasdaq-100-ticker-history](https://github.com/jmccarrell/n100tickers)** (paquete de Python, licencia **MIT**):
- Devuelve el conjunto de tickers del índice **en cualquier fecha**, desde el 1-1-2003 hasta al menos el 17-11-2026, según su README.
- Está mantenido: la última versión es del 20-11-2026.
- Guarda los cambios de cada año en YAML, con lo que se puede auditar.
- Licencia MIT: se puede usar citando el aviso de copyright.

**Uso:** solo en `intelligence/` (los análogos son de Python, CLAUDE.md §1), con la versión **fijada**. Antes de cada carga se registra qué versión se usó.

## 2. Precios: pendiente de verificar y votar

Ninguna fuente gratuita que encontré cumple, verificado, las tres condiciones: **5 años**, **empresas que dejaron de cotizar** y **velas intradía**. Candidatas, **todas sin verificar** en su documentación oficial:

| Fuente | Lo que dicen reseñas o el propio proveedor (sin verificar) | Qué hay que confirmar |
|---|---|---|
| Tiingo | Plan gratuito con tope de símbolos distintos por mes; intradía del feed de IEX | Si el histórico diario incluye empresas que dejaron de cotizar, y el tope real |
| Massive (ex Polygon.io) | Dice cubrir tickers activos y dados de baja; el plan gratuito tendría poca historia | Historia del plan gratuito, intradía y costo del plan pago |
| EODHD | Tiene datos de empresas dadas de baja; el intradía de 5 min y 1 h empezaría en 2020 | Costo y términos de uso |
| Financial Modeling Prep | La lista de empresas dadas de baja sería gratuita; los precios, no confirmado | Precios históricos de empresas dadas de baja en el plan gratuito |
| Yahoo Finance | Muy usada, pero sin API oficial | Términos de uso: **no se usa scraping que los viole** |

Fuentes de las afirmaciones: [comparativa Tiingo vs. Polygon (2026)](https://dev.to/pickuma/tiingo-vs-polygonio-market-data-apis-for-indie-quant-projects-in-2026-2j2e), [Massive](https://massive.com/stocks-data), [EODHD](https://eodhd.com/financial-apis/review-delisted-stock-companies-data) y [FMP](https://site.financialmodelingprep.com/how-to/how-to-handle-delisted-companies-and-historical-symbols-with-a-free-api).

**Propuesta de verificación** (sin pagar nada todavía):
1. Leer la documentación oficial y los términos de Tiingo y Massive.
2. Con la cuenta gratuita, pedir el histórico **diario** de 3 empresas que salieron del índice entre 2021 y 2026 (se sacan del paquete del punto 1) y comparar con otra fuente.
3. Llevar a votación del grupo el resultado y, si hace falta, el costo de un plan pago.

**Resultado del paso 1 (22-11-2026, #10):** ninguna de las cuatro candidatas cumple **gratis**. Tiingo gratis prohíbe guardar los datos. EODHD gratis da un solo año. Massive no tiene un plan gratuito de acciones visible. FMP no se pudo leer. La opción paga que mejor encaja es EODHD. El paso 2 queda sin hacer porque ninguna cuenta gratuita sirve. Detalle y citas en `docs/research/precios-componentes-nasdaq100-2026-11.md`. No cambia la decisión de §4: los análogos v1 usan solo los índices.

## 3. Consecuencia para la spec (a decidir)

La spec §13 compara la **forma de las últimas 32 velas de 15m**. Con fuentes gratuitas lo más probable es tener solo **velas diarias** de 5 años para las empresas que dejaron de cotizar. Opciones:
- **A.** Análogos v1 en velas **diarias** para el universo NASDAQ 100, y de 15m solo para los índices (que vienen de IG, #8).
- **B.** Pagar un proveedor con intradía de 5 años.

Recomendación: **A** para empezar. Se mide si los análogos aportan (fase 5) antes de gastar plata.

## 4. Análogos v1 sin costo (enfoque FI, propuesta del 21-11-2026)

El dueño pidió avanzar sin pagar fuentes. Propuesta concreta para v1:

1. **Universo v1 = los índices que ya opera el sistema** (US100, US500, US30 y DE40), con **velas diarias de 5 años de IG**. IG ya está aprobado como proveedor (ADR 0002), así que no hace falta otra fuente.
   - **Costo:** el cupo semanal de históricos de IG (10.000 puntos). Cinco años de diarias son del orden de 1.300 velas por índice, unos 5.200 puntos para los cuatro: entra en una semana. Se confirma con la primera carga.
   - **Por qué diarias y no de 15m:** cinco años de 15m de un índice que cotiza casi 24 h son unas 120.000 velas, o sea, unas 12 semanas de cupo **por índice**.
   - **Sin sesgo de supervivencia:** un índice no "deja de cotizar", así que este universo no lo tiene.
2. **Componentes del NASDAQ 100:** la composición por fecha ya está resuelta (sección 1). Sus precios siguen pendientes (sección 2). Se suman cuando una fuente gratuita quede verificada y votada. Mientras tanto, los análogos no los usan.
3. **Spec §13:** los análogos v1 comparan la forma de velas **diarias**, no las últimas 32 de 15m. Es la opción **A** de la sección 3, recortada a los índices. Ajustar la §13 va en un PR aparte, aprobado por los dos.

**Falta verificar** en la documentación oficial de IG (labs.ig.com) que `GET /prices` acepte una resolución diaria. No se supone.

## Consecuencias

- Los análogos se pueden construir sin sesgo de supervivencia con un costo cero en componentes.
- La fuente de precios queda abierta hasta verificar y votar. Ningún precio se carga sin fuente documentada.
- Si se elige A, la spec §13 se ajusta en un PR aparte, aprobado por los dos.
