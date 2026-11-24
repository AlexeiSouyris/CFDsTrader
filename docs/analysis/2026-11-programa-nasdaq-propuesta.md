# Programa Nasdaq: especializar la investigación en el CFD del Nasdaq 100 (US100) con contexto externo (propuesta)

Issue #526 · 24-11-2026 · **propuesta para el dueño, no es un pre-registro congelado.** No se corrió ninguna prueba ni backtest, no se gastó dinero en datos y no cambia `config/strategy.toml`, `config/risk.toml`, el runner, el paper ni la spec. Nada de esto es una recomendación de operar: son hipótesis para medir, y los resultados pasados no prometen ningún resultado futuro.

Pedido del dueño (24-11-2026, textual, en #526): "Quiero que este modelo se especialice en Nasdaq y que tome contexto desde afuera para tradear en nasdaq CFDs. Yo creo que si nos metemos mas en un nicho, podemos sacar mejores resultados."

## Resumen para alguien que no sabe de trading

- **Qué se propone.** Dejar de mirar los tres índices de EE. UU. juntos y estudiar **solo el US100** (el CFD del Nasdaq 100), usando información **de afuera** del precio: el calendario de la Reserva Federal (Fed), los datos de inflación (CPI) y de empleo, los resultados trimestrales de las empresas más grandes del índice, las tasas de interés, el dólar y el VIX.
- **Lo honesto primero: especializarse no da más datos, da menos.** Con un solo mercado hay 153 meses (≈ 3 270 días) para probar, igual que antes, pero sin la ayuda de los otros dos índices. Con eso, **solo se puede detectar una ventaja grande**: haría falta algo con un rendimiento por unidad de riesgo (Sharpe) de ≈ 0,86 por año **neto de costos**, que es casi lo mismo que dio simplemente comprar el US100 y quedarse (§2).
- **Lo que ya probamos sobre US100 no funcionó:** la estrategia de 15 minutos pierde (R002, R007) y la "deriva nocturna" que existía hasta 2020 desapareció desde 2021 (R021–R023). El ML con VIX y tasas no mejoró nada (R011, R012).
- **Dónde sí puede ayudar el nicho:** el Nasdaq tiene características propias (pocas empresas gigantes pesan muchísimo, es muy sensible a las tasas, depende de los semiconductores y de los resultados trimestrales). Una idea que use **eso** tiene más sentido que una regla genérica de precio.
- **Propuesta concreta: 4 hipótesis, 1,25 % de presupuesto de error cada una (5 % en total):**
  1. **H1. Antes del anuncio de la Fed** (la "deriva previa al FOMC"): estar comprado en US100 durante las ~24 horas antes del comunicado. **Los datos ya están en la base.**
  2. **H2. Días de CPI y empleo:** estar comprado en US100 la sesión del dato. Falta cargar el calendario del BLS (gratis; falta que el dueño elija cómo acceder).
  3. **H3. Resultados de las gigantes del índice** (Apple, Microsoft, Nvidia, etc.): qué hace el US100 al día siguiente de que una publica resultados después del cierre. **Bloqueada por datos**: hay fuente gratis para la hora de publicación (a verificar), pero no encontramos una gratis para saber, en cada fecha pasada, cuáles eran las más grandes sin mirar el futuro.
  4. **H4. Un modelo de ML chico con contexto** (VIX, tasas, días de eventos), con walk-forward. Expectativa baja: las versiones anteriores no mejoraron.
- **Ideas descartadas, con su razón (§4):** reaccionar a las tasas o al dólar de la madrugada (el Nasdaq ya cotiza a esa hora y lo descuenta en el momento), el régimen de VIX solo (es la familia V, que no pasó) y la deriva de la sesión de Asia y Europa (se apagó en 2021).
- **Recomendación:** empezar solo por **H1** (datos listos, evidencia publicada más fuerte, costo bajo frente al efecto esperado), y **H2** cuando esté el calendario del BLS. H3 y H4 quedan en reserva. Y **no probar dos veces** la misma idea: la familia E del programa 2 (Fed, CPI y empleo sobre los tres índices) y H1/H2 son la misma pregunta; se corre **una sola vez** (§6, decisión 2).

## 0. Punto de partida: lo que ya se sabe del US100

| Fila | Qué se probó | Resultado sobre US100 |
|---|---|---|
| R002 | Estrategia de 15 minutos, futuros 2011–2025 | −0,12 R por operación neto (+0,01 antes de costos; los costos se llevan 0,13 R) |
| R006 → R007 | Walk-forward de la estrategia en uso | 8 ventanas parecían en cero (+0,07 %); con 51 ventanas pierde −6,1 % por ventana (**peor**) |
| R010–R013 | ML v1 y v2 (en US500): contexto, tasas del Tesoro, VIX, análogos | AUC 0,50–0,53: nada mejora a la tabla base |
| R014–R020 | Programas 1 y 2 sobre la cartera US500+US100+US30 | ninguna de las 6 ideas pasa |
| R018 / chequeos #512 | Comprar y mantener el CFD con costos | US100 solo: +1,46 % por mes, Sharpe mensual 0,293 (≈ 1,0 anual), escenario EUR |
| R021 | Deriva nocturna 02:00–03:00 ET | US100 +1,03 pb por día (t 3,42), menor que su costo en el CFD |
| R022 | La misma, por mitades | US100: +1,62 pb (2013–2020) y **+0,08 pb (2021–2025, t 0,17)** |
| R023 | La misma, después de caídas fuertes | US100 +2,05 pb en todo el período; **+0,53 pb desde 2021 (t 0,34)** |

Tres lecturas:

1. **Antes de costos, las reglas de precio de corto plazo están en cero** (R002, R026). El problema no es el índice elegido: es que el precio solo no anticipa nada y los costos del CFD se comen lo poco que hay.
2. **Lo único grande fue la prima de mercado** (comprar y mantener), y en US100 fue la más alta de los tres índices (Sharpe 0,293 mensual frente a 0,245 de US500).
3. **Los efectos publicados se apagan** (la deriva nocturna desde 2021, coherente con McLean y Pontiff, 2016). Cualquier hipótesis nueva tiene que mirarse por mitades.

Por eso el cambio útil no es "otra regla de precio en US100", sino **información que el precio de los últimos 15 minutos no tiene**: fechas de eventos que se conocen de antemano y datos de otros mercados.

## 1. ¿Un nicho da mejores resultados?

### A favor

- **El Nasdaq 100 tiene un contexto propio y medible:**
  - **Concentración:** unas pocas empresas gigantes (Apple, Microsoft, Nvidia, Amazon, Alphabet, Meta, Broadcom, Tesla…) suman una parte muy grande del índice (del orden de la mitad en los últimos años; **el número exacto se confirma con la fuente oficial de Nasdaq**, no se inventa). Sus resultados trimestrales mueven el índice entero, y la fecha de cada uno se conoce antes.
  - **Sensibilidad a las tasas reales:** las empresas de crecimiento valen por ganancias lejanas en el tiempo, así que las tasas de largo plazo (la nota a 10 años, ZN) las afectan más que al Dow (US30).
  - **Semiconductores:** una parte grande del índice depende del ciclo de los chips (Nvidia, Broadcom, AMD, etc.).
  - **Horario:** casi todas las gigantes publican resultados **después del cierre** de Nueva York (16:00 ET). La reacción ocurre en la sesión extendida y en la noche de Asia y Europa, cuando el CFD sigue cotizando.
- **Un modelo enfocado es más simple de entender y de auditar**: menos instrumentos, menos parámetros, menos lugar para el autoengaño.
- **Comparación natural:** si una idea es "propia del Nasdaq" (por ejemplo, los resultados de las gigantes), tiene que ser **más fuerte en US100 que en US30**. Eso da un control gratis: se mide en US30 y US500 como descriptivo, sin voto.

### En contra

- **Menos datos y menos poder estadístico.** Un nicho no agrega años de historia: siguen siendo 153 meses (02-2013 → 10-2025, la validación walk-forward de siempre) o ≈ 3 270 días. Se pierde la ayuda de los otros índices (aunque es poca: están muy correlacionados, ver §2).
- **El Nasdaq 100 es de los mercados más eficientes del mundo.** Sus futuros (NQ) los operan profesionales con datos y velocidad que nosotros no tenemos. Una ventaja que se ve en una vela de 15 minutos con un dato público casi seguro ya está descontada.
- **Lo ya probado sobre US100 dio negativo** (R002, R007) o se apagó (R021–R023). El nicho no cambia los costos del CFD: el spread de US100 es 1,6 puntos en sesión y 2,5 a 5,8 fuera de ella (`docs/research/ig-spreads-2026-11.md`), más la financiación del 3 % anual sobre el nominal si se mantiene de noche.
- **Más contexto = más pruebas = más azar.** Cada variable externa que se agrega es una prueba más. El Sharpe deflactado ya cuenta **354 pruebas** (ronda 3) y un programa nuevo **no** reinicia ese contador.
- **El contexto ya falló una vez:** el ML v2 con tasas del Tesoro y VIX no mejoró nada (R011, R012), aunque fue en US500 y para otra pregunta (tocar TP1 de la estrategia de 15 m).

**Conclusión de esta sección:** especializarse en el Nasdaq **no** hace más probable encontrar una ventaja por sí solo. Ayuda **solo** si la hipótesis usa algo específico del Nasdaq que el precio de corto plazo no tiene. Las hipótesis de §3 se eligieron con ese filtro.

## 2. Poder estadístico con un solo mercado

Cuentas aproximadas, con las reglas de los programas anteriores: prueba unilateral con p < 0,0125 y una probabilidad de 80 % de detectar el efecto si existe (z = 2,24 + 0,84 = 3,08). Son cálculos de papel, no un backtest.

| Muestra | Efecto mínimo detectable | En Sharpe anual |
|---|---|---|
| 153 meses | Sharpe mensual ≥ 3,08 / √153 = **0,249** | ≈ **0,86** |
| ≈ 3 270 días | Sharpe diario ≥ 3,08 / √3270 = **0,054** | ≈ **0,86** |
| ≈ 120 comunicados del FOMC (122 cargados, 2011–2025) | media por evento ≥ **0,28** desvíos de ese evento | — |
| ≈ 360 datos de CPI y empleo (2011–2025) | media por evento ≥ **0,16** desvíos | — |

Qué quiere decir:

- **Una ventaja de US100 tiene que ser casi tan buena como comprar y mantener para que se vea.** Comprar y mantener US100 dio Sharpe mensual 0,293 (≈ 1,0 anual). Una regla con la mitad de eso (≈ 0,5 anual) tendría solo ≈ 30 % de probabilidad de pasar la prueba aunque fuera real.
- **El Sharpe deflactado es la vara más alta.** Con N ≈ 355 pruebas registradas y 153 meses, la fórmula del proyecto (`research::stats::deflated_sharpe`) pone el Sharpe "esperable por azar" en ≈ 0,24 mensual, y para llegar a 0,95 hace falta ≈ **0,37 mensual (≈ 1,3 anual)** con retornos normales. Eso es **más** que comprar y mantener US100. Para estrategias que están poco tiempo en el mercado (eventos), esta vara es muy difícil.
- **Perder los otros dos índices cuesta poco poder:** con correlaciones de ≈ 0,9 entre los tres, promediarlos baja el desvío solo ≈ 3 % (√((1 + 2 × 0,9) / 3) ≈ 0,97). La pérdida real es otra: el criterio "2 de 3 índices" deja de existir y hay que reemplazarlo (§3.5).
- **Ejemplos con los efectos ya observados:**
  - La deriva nocturna de US100 (R021) tenía un Sharpe diario de ≈ 0,06 antes de costos (3,42 / √3270): **apenas** por encima del mínimo detectable, y su costo en el CFD (1,5 a 2,2 pb) era mayor que el efecto (1,03 pb). Desde 2021, cero.
  - **FOMC:** Lucca y Moench (2015) informan ≈ 49 pb en las 24 h antes del anuncio en el S&P 500 (1994–2011; cifra del resumen del artículo, no verificada en esta tarea). Si el desvío de esas 24 h en US100 fuera ≈ 1,3 % (supuesto), serían ≈ 0,38 desvíos: con 120 eventos, ≈ 97 % de poder. **Si el efecto se achicó a la mitad**, el poder cae a ≈ 44 %.
  - **CPI y empleo:** un premio del orden de 0,085 desvíos por día (supuesto, a verificar con Savor y Wilson, 2013) daría solo ≈ 26 % de poder con 360 eventos. **H2 probablemente no tiene poder suficiente**; se dice ahora para no sorprenderse después.

## 3. Hipótesis propuestas

### 3.0 Reglas comunes a todas

**Datos y período.** Futuros NQ de 1 minuto (Databento, en la base desde 2011), armados en velas de 15 m como en las rondas. Validación 02-2013 → 10-2025 (153 meses, las 51 ventanas de `config/data_split.toml`). **El fuera de muestra (desde el 01-11-2025) no se carga ni se mira**; se usaría una sola vez, para un solo candidato congelado, y solo si el dueño lo aprueba.

**Sin mirar el futuro** (`CLAUDE.md` §5b): la decisión se toma al **cierre** de una vela de 15 m con datos publicados **antes** de ese cierre (columna `published_at` de cada fuente) y se ejecuta en la **apertura de la vela siguiente**. Ningún dato vale desde la hora del evento si se publicó después.

**Costos completos del CFD US100** (de `config/instruments.toml`, `config/backtest.toml` y `docs/research/ig-spreads-2026-11.md`; las mismas funciones que las rondas, `engine/crates/backtest/src/costs.rs`):

| Costo | Valor | Fuente |
|---|---|---|
| Spread en sesión (14–20 UTC en invierno) | **1,6 puntos** (medido 1,5; se usa el mayor entre lo medido y lo publicado, regla de #67) | `ig-spreads-2026-11.md`, `instruments.toml` |
| Spread fuera de sesión | 2,5 puntos (00–13 y 23 UTC); 5,8 (21–22 UTC) | `ig-spreads-2026-11.md` |
| Slippage | 0,1 × spread + 1 tick (0,1 punto) por ejecución | `config/backtest.toml` |
| Comisión | 0 (índices de IG) | `costs.rs` |
| Financiación de un largo | tasa de referencia + **3 % anual** sobre el nominal, cada noche (base 360; tres noches el fin de semana), menos la base del futuro (ADR 0015) | `instruments.toml`, `costs.rs` |
| Tasa de referencia | **pendiente con IG** (€STR o SOFR): se corre con los dos escenarios, EUR y USD, como en las rondas | `instruments.toml` (`funding_benchmark` pendiente) |
| Estrés | todos los costos × 1,5 para pasar; × 2 solo se informa | regla de los programas |

**Vara de comparación:** comprar y mantener **el CFD US100** con los mismos costos (`buy_and_hold_returns` / `managed_returns` con peso 1), que dio +1,46 % por mes y Sharpe mensual 0,293 (EUR) en 153 meses (chequeos #512, tabla de la §3).

### 3.1 Presupuesto de error

- **Programa Nasdaq: 5 % en total**, igual que los programas 1, 2 y 3. Un nicho no hace más probable que una idea sea real, así que no se afloja.
- **1,25 % unilateral por hipótesis** (Bonferroni entre 4). Si el dueño aprueba menos hipótesis, el presupuesto que sobra **queda en reserva**, no se reparte entre las otras.
- **Lo que se probó antes sigue contando:** el Sharpe deflactado usa la cantidad de pruebas del registro al momento de congelar (al menos 355; el número exacto se fija al congelar, como en el borrador del programa 3).
- **Choque con presupuestos abiertos:** al programa 2 le quedan 2,5 % de reserva (pensados para la familia E) y el borrador del programa 3 pide 5 % propio. Si los tres siguen vivos a la vez, la tasa total de falsos positivos del proyecto sube. Lo decide el dueño (§6, decisión 2).

### 3.2 Regla de decisión (la misma que los programas anteriores, adaptada a un solo índice)

Una hipótesis pasa solo si cumple **todos** los criterios:

1. **Alfa contra comprar y mantener US100:** regresión de los retornos mensuales netos de la regla sobre los de comprar y mantener US100 (errores HC1, `research::stats::alpha_hc1`); intercepto > 0 con **p < 0,0125 unilateral**. Se usa el alfa (y no "retorno > 0") porque estas reglas están compradas casi siempre que están en el mercado: sin el alfa, ganarían solo por la prima del mercado.
2. **Sharpe deflactado ≥ 0,95** de la ventaja sobre comprar y mantener (alfa + residuo), con N = las pruebas del registro al congelar.
3. **Consistencia:** el alfa es positivo en **las dos mitades** (2013–2020 y 2021–10/2025). Reemplaza al "2 de 3 índices", que con un solo índice no existe. Además, como **descriptivo sin voto**, la misma regla en US500 y US30.
4. **Costos × 1,5:** el alfa sigue > 0.
5. **Frente a comprar y mantener:** la mezcla 50 % comprar y mantener US100 + 50 % la regla tiene un Sharpe mayor que comprar y mantener US100 solo, en los dos escenarios de financiación. (Una regla de eventos está poco tiempo en el mercado: exigirle por sí sola el Sharpe de comprar y mantener no tiene sentido; lo que importa es si **agrega**.)

Qué se hace con el resultado: pasa → al dueño, que decide si se congela y se mira el fuera de muestra; no pasa → se cierra y **no se prueban variantes** sobre estos datos. Cada prueba agrega su fila al registro de resultados.

### 3.3 Las hipótesis

#### H1. Deriva antes del comunicado del FOMC (prioridad 1, datos listos)

- **Idea:** el mercado de acciones sube, en promedio, en las horas **antes** del comunicado programado de la Fed. Fuente: Lucca, D. O. y Moench, E. (2015). *The pre-FOMC announcement drift*. Journal of Finance, 70(1), 329–371. Es la familia E del programa 2, aplicada al US100.
- **Por qué en el Nasdaq:** es el índice más sensible a las tasas; si la incertidumbre sobre la Fed paga un premio, debería verse con más fuerza acá. Hay trabajos que dicen que el efecto se achicó después de su publicación (**a verificar**; por eso el criterio 3 por mitades).
- **Regla (fija, sin parámetros ajustables):** solo reuniones **programadas** (las no programadas no se sabían antes, ADR 0016 §4). Compra en la apertura de la vela de 15 m de las **14:00 ET del día hábil anterior** al comunicado. Venta en la apertura de la vela que empieza **30 minutos antes** de la hora programada del comunicado (por ejemplo 13:30 ET para un comunicado de las 14:00; margen por las horas aproximadas de antes de 2020, que el ADR 0016 trata como ± 15 minutos). Una noche de financiación. Tamaño: igual nominal que comprar y mantener (exposición 1).
- **Datos y publicación real:** tabla `official_releases` (serie `fomc_statement`, **122 comunicados del 26-01-2011 al 10-12-2025, ya en la base**, #520/#521). Cada fila vale desde su `published_at` (el comunicado que anunció el calendario tentativo del año; si no tiene hora, el final de ese día). La regla solo usa una reunión si `published_at` < la vela de entrada.
- **Costo frente al efecto:** ida y vuelta en sesión ≈ 1,6 + 2 × 0,26 = 2,1 puntos (≈ 1 a 5 pb según el nivel del índice) más una noche de financiación (≈ 1 a 2 pb). Unos 3 a 7 pb frente a un efecto publicado de ≈ 49 pb: **el costo no es el problema; el problema es si el efecto sigue vivo.**
- **Poder:** ≈ 97 % si el efecto es como el publicado; ≈ 44 % si es la mitad (§2).

#### H2. Días de CPI y empleo (prioridad 2, falta el calendario del BLS)

- **Idea:** los días con anuncios macro programados tienen retornos medios más altos (premio por el riesgo del anuncio). Fuente: Savor, P. y Wilson, M. (2013). *How much do investors care about macroeconomic risk? Evidence from scheduled economic announcements*. Journal of Financial and Quantitative Analysis, 48(2), 343–375 (página no leída en esta tarea).
- **Regla:** compra en la apertura de la vela de 15 m de las **08:00 ET** del día del CPI o del informe de empleo (el dato sale a las 08:30 ET; se entra antes, porque reaccionar después es llegar tarde, programa 2 §2). Venta en la apertura de la vela de las **15:45 ET** del mismo día (sin noche, sin financiación). Si el mismo día hay CPI y empleo, cuenta una sola vez.
- **Datos y publicación real:** calendario del BLS según el ADR 0016 (`published_at` = la hora de embargo del informe anterior de la misma serie, que anuncia la fecha siguiente). **No está cargado:** el ADR dejó abierta la forma de acceso al BLS (§1.3), que decide el dueño. Gratis.
- **Costo:** la entrada es fuera de sesión (spread 2,5 puntos) y la salida en sesión (1,6): ≈ 2,7 puntos de ida y vuelta con slippage, ≈ 1 a 6 pb.
- **Poder: bajo** (≈ 26 % con el efecto supuesto, §2). Se propone igual porque es la otra mitad de la familia E y es barata, pero **el dueño debería saber que lo más probable es que no pase aunque exista**.

#### H3. Resultados trimestrales de las gigantes del índice (bloqueada por datos)

- **Idea:** cuando una de las empresas más grandes del índice publica resultados después del cierre, el US100 se mueve en la noche. La hipótesis es que **ese movimiento nocturno sigue en la misma dirección durante la sesión del día siguiente** (la información de una empresa enorme tarda en llegar al resto del índice). Fuente del tipo de idea: Savor, P. y Wilson, M. (2016). *Earnings announcements and systematic risk*. Journal of Finance, 71(1), 83–138 (**referencia a verificar**: no se leyó). Evidencia directa sobre el índice: **no encontramos**; es la más especulativa de las cuatro.
- **Regla (borrador):** para cada anuncio después del cierre de una empresa del grupo "gigantes", se mide el retorno de US100 desde el cierre de contado (16:00 ET) hasta la vela de las 09:15 ET del día siguiente. Si es positivo, compra en la apertura de la vela de las 09:30 ET; si es negativo, venta. Cierre en la apertura de la vela de las 15:45 ET. Sin noche.
- **Datos y publicación real:**
  - **Hora del anuncio:** la SEC (EDGAR) guarda los formularios 8-K con el ítem 2.02 ("Results of Operations") y su **hora de aceptación**, con historia desde mucho antes de 2011. Es gratis. Se tomaría esa hora como `published_at` (si el comunicado de prensa salió unos minutos antes, se pierde un poco de efecto, pero no se mira el futuro). **A verificar**: que todas las gigantes presenten el 8-K a la misma hora del comunicado, y la política de acceso de la SEC (pide un User-Agent con datos de contacto; qué datos poner lo decide el dueño).
  - **Quiénes eran "las gigantes" en cada fecha:** elegirlas con la lista de hoy sería mirar el futuro (Nvidia era chica en 2013). Hace falta el **peso de cada empresa en el índice en cada fecha** o su capitalización. El módulo `universe` (#10) tiene los **tickers** por fecha, sin pesos ni precios. **No encontramos una fuente gratis con historia de pesos**; la opción paga conocida para precios de componentes es EODHD (`docs/research/precios-componentes-nasdaq100-2026-11.md`), con precio **a confirmar**. Además habría que tener la cantidad de acciones de cada empresa en cada fecha (a confirmar si EODHD la trae).
  - Lo que sí hay: las velas del US100 (NQ) para medir la reacción del índice, sin precios de acciones.
- **Costo:** entrada y salida en sesión, ≈ 2,1 puntos de ida y vuelta.
- **Estado:** **no se puede pre-registrar** hasta resolver los pesos. Si el dueño no quiere pagar datos, H3 se cierra y su 1,25 % queda en reserva.

#### H4. Modelo de ML chico con contexto externo (prioridad 3)

- **Idea:** un modelo que, con pocas variables de contexto conocidas antes de la apertura, prediga si la sesión de contado del US100 (09:30–16:00 ET) termina arriba o abajo, y opere solo cuando la probabilidad está lejos de 50 %.
- **Variables (todas con su `published_at`, fijadas antes de mirar; ninguna se agrega después):**
  1. VIX del día hábil anterior (vale desde las 09:30 ET del día siguiente, ADR 0012) y su cambio en 5 días;
  2. cambio diario de la nota a 10 años (ZN, vela diaria del día anterior, que cierra a medianoche UTC);
  3. retorno nocturno del propio US100 (cierre de contado → 09:15 ET);
  4. marcas de día FOMC, CPI o empleo (solo si `published_at` < la decisión);
  5. volatilidad realizada de 20 días del US100.
- **Modelo:** regresión logística (como la base B1 del ML v1: el más simple que se puede auditar), entrenada con **walk-forward 24 + 3 meses**, paso 3, purga y embargo como `docs/spec/ml-v1.md` §5. El umbral para operar (por ejemplo probabilidad > 55 %) se fija **antes** de correr y **no** se ajusta. El fuera de muestra no se toca.
- **Por qué está última:** el ML v1 y v2 con VIX y tasas no mejoró nada (R010–R013). Esta versión cambia la pregunta (dirección de la sesión, no TP1 de la estrategia de 15 m) y el instrumento, pero la expectativa es baja.
- **Límite de datos:** el VIX y la curva del Tesoro guardados empiezan en **2019**. Con 24 meses de entrenamiento, la validación sería 2021–2025 (≈ 19 ventanas): poca muestra. Antes de H4 conviene cargar el VIX desde 2011 (el CSV oficial de Cboe trae desde 1990; el dueño ya aceptó sus términos en el ADR 0012) y la curva del Tesoro anterior a 2019 (CSV oficial por año; que respondan los años anteriores a 2019 está **a verificar**). Con ZN se puede usar la vela diaria, que ya está desde 2011.
- **Dónde vive:** el modelo en `intelligence/` (Python, `CLAUDE.md` §1); los retornos y costos en Rust (`research`), como el ML v1.

### 3.4 Qué se reutiliza

- **Familia E del programa 2** (`2026-11-programa-2-propuesta.md` §1.1 b): H1 y H2 **son** la familia E, aplicada solo a US100. No se corre dos veces (§6, decisión 2).
- **Código de las rondas:** `cfds_backtest::research` (`run_plans`, `buy_and_hold_returns`, `managed_returns`, `alpha_hc1`, `deflated_sharpe`, `window_returns`) y la orquestación de `examples/common/research_round.rs`. Haría falta una función pura nueva para las reglas de eventos (`event_window_plans` o similar), con tests.
- **Calendario oficial:** `official_releases` (ADR 0016, #520/#521) para el FOMC; la parte del BLS falta.
- **ML de `intelligence/`:** la estructura del ML v1 (walk-forward, purga, registro de experimentos, criterio fijado antes, `docs/spec/ml-v1.md`) y la tabla base B1 como comparación.
- **Resultados previos como control:** R002 y R007 muestran que la regla de precio de 15 m no sirve en US100; R021–R023 que la deriva nocturna se apagó. Ninguna hipótesis nueva los repite.

### 3.5 Lo que cambia respecto de los programas anteriores

| Punto | Programas 1–2 | Programa Nasdaq | Por qué |
|---|---|---|---|
| Universo de la prueba | cartera US500 + US100 + US30 | **solo US100**; US500 y US30 como descriptivo sin voto | es lo que pidió el dueño |
| Consistencia | dos mitades y 2 de 3 índices | **dos mitades**; los otros índices sin voto | con un índice no hay "2 de 3" |
| Vara | Sharpe de la regla ≥ comprar y mantener (o alfa en V) | **alfa** + mezcla 50/50 ≥ comprar y mantener US100 | las reglas de eventos están poco tiempo en el mercado |
| Presupuesto | 5 % por programa | **5 %**, 1,25 % por hipótesis | igual exigencia |

## 4. Ideas evaluadas y descartadas (con su razón)

| Idea | Por qué no entra como hipótesis |
|---|---|
| **Tasas (ZN) o dólar (6E, 6J) de la madrugada para operar la apertura** | El NQ **también cotiza de madrugada**, al mismo tiempo que ZN y las monedas. Lo que cambian las tasas a las 03:00 ET ya está en el precio del US100 a las 03:00 ET: no hay atraso que aprovechar con velas de 15 m. Además, las velas diarias que tenemos de ZN y monedas cierran a **medianoche UTC** (`config/databento_daily.toml`), así que no sirven para medir "la madrugada antes de la apertura"; harían falta velas de 1 minuto de ZN (costo **a confirmar** con `--estimate` de Databento, que es gratis; como referencia, el oro 1 m 2011–2026 se estimó en USD 20,13). Queda solo como **variable** de H4, con la vela diaria. |
| **Régimen de VIX o VXN como filtro** | Es la familia V (exposición según la volatilidad), que no pasó (R019). El VIX es la volatilidad esperada del S&P 500; el VXN la del Nasdaq 100 (Cboe la publica; historia y términos **a verificar**, iguales problemas que el VIX, ADR 0012). Queda solo como variable de H4. |
| **Deriva de la sesión de Asia y Europa** | Ya medida: R021–R023. Se apagó desde 2021 y su costo en el CFD es mayor que el efecto. |
| **Reacción a noticias de empresas en el momento** | Los profesionales leen la noticia en milisegundos; nuestro sistema llega 15 minutos tarde (programa 2 §2). Y no hay archivo de noticias con hora. |
| **Deriva después de resultados en las acciones (PEAD)** | Opera acciones, no el índice (cambia el universo) y en las grandes casi desapareció (Martineau, 2022, citado en el programa 2). |
| **Otra regla de precio de 15 m solo en US100** | R002 y R007: antes de costos está en cero. |

## 5. Datos: qué hay, qué es gratis y qué cuesta

Consultado en la base el 24-11-2026, **solo lectura** (solo fechas extremas y cantidad de filas, ningún precio).

| Dato | Para | Estado | Desde | Costo |
|---|---|---|---|---|
| Futuros NQ 1 m (US100) | todas | **en la base** (`futures_bars_1m`) | 02-01-2011 | ya pagado (ADR 0011) |
| Futuros ES e YM 1 m | control sin voto | **en la base** | 2011 | ya pagado |
| Comunicados del FOMC con hora y `published_at` | H1, H4 | **en la base** (`official_releases`, 122 filas) | 26-01-2011 | gratis |
| Calendario de CPI y empleo (BLS) | H2, H4 | **falta**; falta elegir la forma de acceso (ADR 0016 §1.3) | — | gratis |
| VIX diario | H4 | **en la base** (`vix_daily`, 2017 filas) | 02-01-2019 | gratis (CSV de Cboe desde 1990; términos aceptados por el dueño, ADR 0012) |
| VIX 2011–2018 | H4 | falta cargar | — | gratis (mismo CSV) |
| VXN (volatilidad del Nasdaq 100) | descartado / H4 opcional | no está | — | a verificar (Cboe) |
| Curva del Tesoro diaria | H4 opcional | **en la base** (`treasury_yields`) | 02-01-2019 | gratis; años anteriores a verificar |
| ZN y ZB diarios (medianoche UTC) | H4 | **en la base** (`futures_bars_1d`) | 02-01-2011 | ya pagado (USD 0,49 los 10 mercados, #522) |
| 6E diario | H4 opcional | en la base **solo hasta el 31-12-2019** | 2011 | ya pagado; revisar por qué la carga está incompleta |
| 6J, 6B, 6A, SI, CL, HG diarios | no se usan | **no aparecen** en `futures_bars_1d` al 24-11 | — | ya incluidos en los USD 0,49 aprobados; revisar la carga |
| NKD diario (Nikkei en dólares) | no se usa | en la base | 2011 | ya pagado |
| ZN o 6E de 1 minuto | descartado | no está | — | **a confirmar** con `--estimate` (gratis) antes de pedir nada |
| Hora de publicación de resultados (8-K, ítem 2.02, SEC EDGAR) | H3 | no está | — | gratis; **a verificar** (forma de acceso y política de la SEC) |
| Fecha **anunciada** de los resultados (antes de que ocurran) | H3 (variantes "antes del anuncio") | no está | — | **no encontramos fuente gratis con historia**; a confirmar |
| Pesos de cada empresa en el Nasdaq 100 por fecha | H3 | no está | — | **no encontramos fuente gratis**; pago a confirmar |
| Precios y acciones en circulación de los componentes | H3 (para calcular pesos) | no está | — | pago (EODHD); precio **a confirmar** |
| Tickers del Nasdaq 100 por fecha | H3 | módulo `universe` (#10, MIT) | — | gratis |

## 6. Decisiones para el dueño

1. **¿Aprueba el programa Nasdaq como propuesta**, con sus 4 hipótesis y 5 % de presupuesto (1,25 % cada una)? O solo algunas (lo recomendado: **H1 ahora, H2 cuando esté el BLS**; H3 y H4 en reserva). La aprobación tiene que quedar escrita en #526; el agente no la da por hecha. Después se escribe el **pre-registro** (congelado con fecha, hora y hash) antes de correr nada.
2. **No probar la misma idea dos veces.** H1/H2 son la familia E del programa 2. Opciones: (a) correr E **una sola vez, sobre US100**, con cargo a la reserva de 2,5 % del programa 2, y cerrar esa reserva; (b) correr E sobre la cartera de tres índices en el programa 2 y sacar H1/H2 de este programa. Recomendación: **(a)**, porque es lo que pidió el dueño y no gasta presupuesto nuevo. En cualquier caso, decir qué pasa con los presupuestos abiertos (2,5 % del programa 2, 5 % del borrador del programa 3 y 5 % de este).
3. **Acceso al BLS** (ADR 0016 §1.3) para cargar CPI y empleo: necesario para H2 y para las marcas de H4.
4. **H3: ¿se paga por datos de componentes?** Sin los pesos del índice en cada fecha, H3 no se puede probar sin mirar el futuro. Si sí: pedir precio a EODHD (o el proveedor que elija), con su ADR de proveedor de datos (`CLAUDE.md` §6). Si no: H3 se cierra.
5. **Acceso a EDGAR** (SEC) para la hora de los resultados: qué User-Agent y contacto usar (no se usa ningún dato personal sin el OK del dueño).
6. **Cargar el VIX 2011–2018** (gratis, mismo CSV) y revisar por qué la carga diaria de #522 tiene 6E solo hasta 2019 y no tiene 6J, 6B, 6A, SI, CL ni HG. Solo hace falta para H4.
7. **Expectativas:** aceptar por escrito que, con un solo índice, solo se detectan ventajas grandes (≈ 0,86 de Sharpe anual neto) y que el Sharpe deflactado pide ≈ 1,3 anual. Lo más probable, viendo R001–R026, es que ninguna hipótesis pase; en ese caso, la conclusión útil sería que **comprar y mantener US100 con menos costo** (programa 2 §1.4) pesa más que cualquier señal.
8. **Nada de esto toca dinero real** (`CLAUDE.md` §5). Aunque algo pasara, operar una regla de eventos pide un cambio de spec (otros horarios, mantener una noche), que se aprueba aparte.

## Límites

- Las citas de Lucca y Moench (2015) y Savor y Wilson (2013) son las mismas del programa 2 (no se releyeron); Savor y Wilson (2016) y la idea de que la deriva previa al FOMC se achicó **no se verificaron**. La cifra de ≈ 49 pb es del resumen de Lucca y Moench, de memoria.
- El desvío de 24 h del US100 (≈ 1,3 %) y el tamaño del premio de CPI/empleo son **supuestos** para el cálculo de poder, no medidos.
- El peso de las gigantes en el índice no se verificó con la fuente oficial de Nasdaq.
- Los costos del CFD son los del catálogo (`config/instruments.toml`), con la tasa de referencia de la financiación pendiente con IG (dos escenarios).
- No se corrió ninguna prueba: el registro de resultados no cambia.
