# 0012 – Rendimientos del Tesoro de EE. UU. y VIX como contexto macro

- **Estado:** aceptada, para el Tesoro y para el VIX. El VIX entra **por decisión expresa del dueño**, contra los términos de Cboe (ver §2).
- **Fecha:** 2026-11-23
- **Aprobada por:** el dueño (modo de un solo constructor, `CLAUDE.md`), el 23-11-2026, con las palabras "Si, hace todo", sobre el plan de los "8 pilares" (pilares 3, 6 y 8). La aprobación la informó el agente coordinador. Para el VIX, que contradice el [ADR 0004](0004-fuentes-de-sentimiento.md), el dueño decidió aparte el mismo día, con estas palabras: "ni hace falta que leas los términos de nada porque es un proyecto solo para mí" y "no me importan los terminos que ponen". También la informó el agente coordinador.
- **Issue:** #260
- **Reemplaza:** la parte del [ADR 0004](0004-fuentes-de-sentimiento.md) que dejó el VIX fuera de v1 (decisión 4), **solo para los cierres diarios del VIX**. El ratio put/call de Cboe y el resto del ADR 0004 siguen igual.
- **Relacionados:** ADR 0003 §C (capas de contexto), #79 (€STR, `cfds_intel.rates`)

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

### 2. VIX: se ingesta por decisión del dueño

**Los términos de Cboe no lo permiten.** Los vigentes (actualizados el 16-11-2022) dejan ver, imprimir y "download one copy of the Materials for your personal non-commercial use". Sin consentimiento escrito prohíben "store either in hard copy or in an electronic retrieval system", crear obras derivadas y redistribuir. Guardar el VIX en PostgreSQL es guardarlo en un sistema de recuperación electrónica.

**El dueño decidió incluirlo igual** (citas arriba). **La decisión y el riesgo son suyos**, no del proyecto ni de los agentes. Reglas para acotar el uso:

1. **Uso solo personal**, en simulación, sin fines comerciales.
2. **Sin redistribución:** los datos del VIX viven **solo en la base del Mac mini**. Nunca van al repo (ni en tests, ni en mocks, ni en ejemplos), ni a otra persona, ni a un servicio externo. Los tests usan números inventados.
3. **Solo el CSV oficial** de cboe.com (`https://cdn.cboe.com/api/global/us_indices/daily_prices/VIX_History.csv`, que redirige a `cdn-api.cboe.com`), con un `User-Agent` que dice qué es. **Sin esquivar nada:** si responde 403 o un desafío anti-bots, el comando se frena con error y no reintenta.
4. **Una lectura por día hábil**, el mínimo necesario.
5. **Si el dueño cambia de opinión o Cboe lo pide**, se borran los datos (los términos dicen "you must destroy promptly all Materials obtained from the Website") y se deja de leer.

**Ingesta** (PR aparte, apilado sobre el del Tesoro):
- **Contrato propio `vix_daily.v1`** (`ref_date`, `published_at`, `open`, `high`, `low`, `close`). No reusa `treasury_yield.v1`: el VIX es el nivel de un índice con apertura, máximo, mínimo y cierre, no un rendimiento por plazo.
- **Tabla de solo agregar**, como la del Tesoro.
- **`published_at` = el cierre del día más una holgura documentada.** El VIX cierra a las 16:15 ET, y Cboe no documenta a qué hora actualiza el CSV. En el histórico, cada valor vale recién desde el **día hábil siguiente de la serie a las 09:30 ET**, la misma regla que el Tesoro. El último valor de cada lectura vale desde que se leyó. Nunca antes de las 16:15 ET del día del valor.

## Alternativas consideradas

- **Reusar `reference_rate.v1` para el Tesoro:** descartado, por lo explicado en §1.2.
- **Usar el valor del Tesoro el mismo día a las 18:00 ET:** más fiel cuando el Tesoro cumple su horario, pero con riesgo de mirar el futuro los días en que se atrasa. Se eligió el día hábil siguiente a las 09:30 ET para el histórico.
- **El feed XML del Tesoro:** tiene los mismos datos. El CSV es más simple y es el formato de descarga que ofrece la página.
- **VIX desde terceros** (sitios o repositorios que lo redistribuyen): descartado. Si se usa, es de la fuente oficial.
- **Pedir permiso a Cboe** (`permissions@cboe.com`) o **seguir sin VIX** con la volatilidad realizada: eran las opciones de la primera versión de este ADR. El dueño eligió incluirlo sin pedir permiso.

## Consecuencias

- Hay curva del Tesoro diaria desde 2019, sin costo y con fuente oficial.
- En el histórico, cada valor llega con un día hábil de atraso respecto de la publicación habitual. Es el costo de no mirar el futuro.
- Si el Tesoro agrega una columna (como pasó con 1,5 meses en 2025), el comando **falla a propósito** hasta que se agregue el plazo al código y al contrato. Nunca se ignora en silencio.
- Hay VIX diario desde 1990, **solo en la base del Mac**. El uso va contra los términos de Cboe, por decisión y bajo responsabilidad del dueño. Si Cboe bloquea la descarga, se frena y se avisa.
