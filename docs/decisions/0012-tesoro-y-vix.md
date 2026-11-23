# 0012 – Rendimientos del Tesoro de EE. UU. y VIX como contexto macro

- **Estado:** aceptada **para el Tesoro**. **El VIX queda fuera**, como dice el ADR 0004, hasta que el dueño decida (ver §2).
- **Fecha:** 2026-11-23
- **Aprobada por:** el dueño (modo de un solo constructor, `CLAUDE.md`), el 23-11-2026, con las palabras "Si, hace todo", sobre el plan de los "8 pilares" (pilares 3, 6 y 8). La aprobación la informó el agente coordinador. La parte del VIX contradice el [ADR 0004](0004-fuentes-de-sentimiento.md), y por eso necesita una decisión propia del dueño.
- **Issue:** #260
- **Relacionados:** ADR 0003 §C (capas de contexto), ADR 0004 (fuentes de sentimiento; VIX fuera de v1), #79 (€STR, `cfds_intel.rates`)

## Contexto

El plan de los "8 pilares" pide contexto macro diario: tasas de EE. UU. (la curva y la pendiente 2–10 años) y volatilidad implícita (VIX). Las dos son series diarias con fuente oficial y sin costo. Como todo el contexto, **se registran y se miden, pero no suman al puntaje** hasta validarse en la fase 5 (ADR 0003 §C).

Lo que se comprobó el 23-11-2026:

### Tesoro de EE. UU.: "Daily Treasury Par Yield Curve Rates"

| Punto | Dato | Fuente |
|---|---|---|
| Qué es | Rendimientos par diarios por plazo: 1, 1,5 (desde 2025), 2, 3, 4 (desde 2022) y 6 meses, y 1, 2, 3, 5, 7, 10, 20 y 30 años | [Interest rates, Daily Treasury Par Yield Curve Rates](https://home.treasury.gov/resource-center/data-chart-center/interest-rates/TextView?type=daily_treasury_yield_curve&field_tdr_date_value=2024) |
| Formato | Un CSV oficial por año, el del botón "Download CSV": `…/daily-treasury-rates.csv/{año}/all?type=daily_treasury_yield_curve&field_tdr_date_value={año}&page&_format=csv`. Responde 200 para 2019 a 2026, con las filas de la más nueva a la más vieja. Las columnas cambian con los años, y un plazo que ese día no existía viene vacío. También hay un feed XML (Atom) con los mismos datos | Ídem |
| Hora de los precios | "obtained by the Federal Reserve Bank of New York at or near 3:30 PM each trading day" | [Treasury Yield Curve Methodology](https://home.treasury.gov/policy-issues/financing-the-government/interest-rate-statistics/treasury-yield-curve-methodology) (revisada el 18-2-2025) |
| Hora de publicación | "usually available at Treasury's interest rate website ... by 6:00 PM Eastern Time each trading day, but may be delayed due to system problems or other issues" | Ídem |
| Términos | Son datos publicados por el Departamento del Tesoro. Las obras del gobierno federal de EE. UU. no tienen copyright (17 U.S.C. § 105). La página de políticas del sitio no trae términos de uso ni limita la reutilización de los datos | [Site Policies and Notices](https://home.treasury.gov/subfooter/site-policies-and-notices) |

### Cboe: VIX

| Punto | Dato | Fuente |
|---|---|---|
| Qué es | Cierres diarios del VIX desde 1990: "VIX Index data for 1990 to present (Updated Daily)". El CSV `VIX_History.csv` (columnas `DATE,OPEN,HIGH,LOW,CLOSE`) responde 200 | [VIX, históricos](https://www.cboe.com/tradable_products/vix/vix_historical_data/) |
| Términos | Se permite ver, imprimir y "download one copy of the Materials for your personal non-commercial use". Se prohíbe, sin consentimiento escrito, "store either in hard copy or in an electronic retrieval system", crear obras derivadas o redistribuir. "Materials" incluye las bases de datos del sitio | [Terms and Conditions for Use of Cboe Websites](https://www.cboe.com/terms) (actualizados el 16-11-2022) |
| Permiso | Para usar datos de Cboe hay que pedir autorización a `permissions@cboe.com`, y una aprobación exige firmar una licencia | [Use of Cboe Content](https://www.cboe.com/use-of-content) |

## Decisión

### 1. Tesoro: se ingesta

1. **Fuente:** solo el CSV oficial por año de home.treasury.gov. Nada de scraping ni de terceros que lo redistribuyan.
2. **Contrato nuevo `treasury_yield.v1`** (`tenor`, `ref_date`, `published_at`, `yield_pct`). No se reusa `reference_rate.v1`: ese contrato es para los tipos de la financiación nocturna. Su `series` tiene el mismo nombre que `[funding_benchmarks]` de `config/instruments.toml`, y el motor lo lee para el costo de cada noche. Mezclar ahí la curva del Tesoro confundiría las dos cosas.
3. **Tabla `treasury_yields`** (migración 0025), **de solo agregar**. Una corrección del Tesoro es una fila nueva publicada cuando se leyó.
4. **`published_at`, sin mirar el futuro:** el Tesoro publica "usually … by 6:00 PM ET", pero puede atrasarse.
   - **Histórico:** cada valor vale recién desde el **día hábil siguiente de la serie a las 09:30 ET**, antes de la apertura de Nueva York. Así se cubren los atrasos de la misma noche. El día hábil siguiente es la fecha de la fila siguiente; la serie solo tiene días hábiles del Tesoro.
   - **Último valor de cada lectura:** vale desde que se leyó. El comando diario de las 18:30 ET le pone la hora real de lectura, y esa hora no cambia después.
   - **Nunca antes de las 15:30 ET del día del valor:** ahí la Fed de Nueva York toma los precios. Lo controlan el código y un `CHECK` de la tabla.
5. **Comando idempotente:** `uv run python -m cfds_intel.treasury [desde_año]` (`config/treasury.toml`). Carga desde 2019.
6. **Uso:** contexto. La pendiente 2–10 años, por ejemplo, se calcula después en el lenguaje que corresponda según `CLAUDE.md` §1, con su propia definición en la spec antes de mirar los datos. Este ADR solo trae el dato.

### 2. VIX: sigue fuera de v1 (lo decide el dueño)

Los términos vigentes de Cboe **no permiten** guardar el VIX en una base de datos ("store … in an electronic retrieval system") sin consentimiento escrito, y una autorización exige una licencia. Es el mismo motivo por el que el ADR 0004 lo dejó fuera. El pedido de este plan no cambia los términos, así que **no se descarga ni se guarda ninguna serie de Cboe**. Opciones para el dueño:

1. **Pedir permiso a Cboe** (`permissions@cboe.com`) para un uso personal, no comercial y en simulación. Si lo dan por escrito, se agrega en un PR aparte con un contrato `vix_close.v1`, con las mismas reglas que el Tesoro.
2. **Seguir sin VIX** y usar la volatilidad realizada, que ya existe (ADR 0004, spec §9.1). Es otra medida, pero sale de nuestros propios precios.
3. **Una fuente con licencia que lo permita**, si aparece, en su propio ADR.

Hasta esa decisión, el VIX queda como estaba en el ADR 0004.

## Alternativas consideradas

- **Reusar `reference_rate.v1` para el Tesoro:** descartado, por lo explicado en §1.2.
- **Usar el valor del Tesoro el mismo día a las 18:00 ET:** más fiel cuando el Tesoro cumple su horario, pero con riesgo de mirar el futuro los días en que se atrasa. Se eligió el día hábil siguiente a las 09:30 ET para el histórico.
- **El feed XML del Tesoro:** tiene los mismos datos. El CSV es más simple y es el formato de descarga que ofrece la página.
- **VIX desde terceros** (sitios o repositorios que lo redistribuyen): descartado. Lo descarta también el ADR 0004.

## Consecuencias

- Hay curva del Tesoro diaria desde 2019, sin costo y con fuente oficial.
- En el histórico, cada valor llega con un día hábil de atraso respecto de la publicación habitual. Es el costo de no mirar el futuro.
- Si el Tesoro agrega una columna (como pasó con 1,5 meses en 2025), el comando **falla a propósito** hasta que se agregue el plazo al código y al contrato. Nunca se ignora en silencio.
- El VIX no está. Si el dueño elige la opción 1 y Cboe autoriza, entra en un PR aparte.
