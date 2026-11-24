# Programa de investigación 2: cambiar la pregunta (propuesta)

Issue #493 · 24-11-2026 · **propuesta para el dueño**. No cambia `config/strategy.toml`, `config/risk.toml`, el runner, el paper ni la spec. Nada de esto es una recomendación de operar: son hipótesis para medir, y los resultados pasados no prometen ningún resultado futuro.

Pedido del dueño (24-11-2026, textual): "me gustaria que invesitguemos ese cambio de pregunta y tomemos otra perspectiva tambien para ver como nos va. Si tenes alguna que otra sugerencia sobre como podemos sacar ventaja, tambien es buena idea". Dio como ejemplo la reacción de una acción (NVDA) a noticias sobre su CEO. **Ese hecho no se verificó y no se usa como dato**: se toma solo como el *tipo* de idea (operar por eventos o noticias).

## Resumen para alguien que no sabe de trading

- **Lo que ya sabemos.** Probamos cuatro formas de elegir "cuándo entrar y cuándo salir" en los índices de EE. UU. (rondas 1 y 2) y la estrategia actual de 15 minutos. **Ninguna ganó después de pagar los costos** del CFD. En el mismo período, simplemente **comprar el índice y quedarse** tuvo un rendimiento por unidad de riesgo (Sharpe) de ≈ 0,9 por año, tres veces el de la mejor regla (≈ 0,3).
- **El cambio de pregunta.** En lugar de preguntar "¿en qué vela de 15 minutos entro?", este programa pregunta cosas distintas:
  1. **¿Cuánto tener invertido?** Mantener el índice, pero con **menos** posición cuando el mercado está muy agitado y **más** cuando está calmo (Moreira y Muir, 2017). No adivina la dirección: maneja el riesgo.
  2. **¿Cuánto cuesta estar?** Gran parte de lo que se pierde es costo del CFD (spread y la tasa de financiación de 3 % anual). Comparar con un futuro chico (micro E-mini) puede cambiar más el resultado que cualquier señal.
  3. **¿Hay momentos especiales?** Días de la Fed, del dato de inflación (CPI) o de empleo; la reacción de una empresa a sus resultados o a una noticia; la "deriva nocturna".
  4. **¿Lo que se llama "manipulación" deja una huella aprovechable?** El informe #492 propone dos ideas medibles: la aceleración al cruzar un número redondo (M1) y la diferencia entre barrer y romper el máximo o mínimo del día anterior (M2).
- **Lo que se puede probar ya, con los datos que tenemos:** la exposición manejada por volatilidad (familia **V**) y el número redondo (familia **M1**). Son la **ronda 3**.
- **Lo que necesita datos nuevos** (y queda propuesto, sin correr): los eventos de la Fed, CPI y empleo (hace falta el calendario histórico), las acciones sueltas como NVDA (hacen falta precios de acciones, fechas de resultados y un archivo de noticias con hora; además **cambia el universo del proyecto, y eso lo decide el dueño**) y el régimen de VIX (el VIX guardado empieza en 2019).
- **Por qué casi todas las "ventajas" con noticias no sirven para un minorista:** los profesionales leen la noticia y operan en milisegundos. Lo que queda para alguien lento son solo los efectos **lentos** (días o semanas), que existen pero son chicos (§2).
- **Las reglas para no engañarnos siguen:** presupuesto de error nuevo de 5 % para este programa, una prueba por familia, todo pre-registrado antes de correr, y el tramo fuera de muestra (desde el 01-11-2025) **sigue sin tocarse**.

## 0. Punto de partida (lo que cerraron las rondas 1 y 2)

| Estudio | Resultado | Fuente |
|---|---|---|
| Estrategia de 15m (US500, US100, US30, 2011–2025) | ≈ 0 R antes de costos; los costos se llevan 0,10–0,18 R por operación | `2026-11-diagnostico-us-futuros.md` (#461) |
| ML v1 y v2 | AUC 0,50–0,53: ni contexto, ni tasas, ni VIX, ni análogos predicen | `2026-11-ml-v1-us500.md`, `2026-11-ml-v2-us500.md` |
| Ronda 1: ruptura del rango de apertura (C), cambio de mes (D1) | C pierde (−0,50 % por mes); D1 +0,03 % por mes, no significativo | `2026-11-ronda1-orb-cambio-de-mes.md` (#472) |
| Ronda 2: tendencia mensual (A), momentum intradía (B) | A +0,08 % por mes, Sharpe ≈ 0,30 frente a ≈ 0,90 de comprar y mantener; B pierde (−2,0 % por mes) | `2026-11-ronda2-tendencia-y-momentum-intradia.md` (#486) |

Dos lecturas guían este programa:

1. **Los efectos con el signo correcto (D1 y A) son de 0,03–0,05 R por operación antes de costos.** Con los costos de un CFD, las reglas que entran y salen seguido no tienen margen.
2. **La prima de riesgo del mercado (comprar y mantener) fue lo único grande** en 2013–2025. La pregunta útil deja de ser "cuándo entrar" y pasa a ser "cómo tener esa prima más barata y con menos riesgo", más algunos momentos con una hipótesis económica clara.

## 1. Preguntas nuevas (candidatas)

Para cada una: la fuente, qué datos tenemos (consultados en la base, **solo lectura**, 24-11-2026) y qué costaría probarla.

### Inventario de datos (base real, solo lectura)

| Dato | Qué hay | Desde | Sirve para |
|---|---|---|---|
| Futuros ES, NQ, YM (1m, 15m, 1h) | Databento | 01-2011 | todo lo de índices |
| Futuros RTY | Databento | 07-2017 | descriptivo |
| Futuros GC (oro) | Databento | 01-2011 | costos de IG pendientes |
| Futuros FDAX | Databento | 03-2025 | no alcanza |
| VIX diario (`vix_daily`) | 2017 días | 01-2019 | solo 2021–2025 en validación |
| Tasas del Tesoro | | 2019 (ML v2) | ídem |
| Calendario económico (`economic_events`) | 73 eventos de Forex Factory | **la semana del 23-11-2026** | **no hay historia** |
| Noticias (`news_items`) | 78: comunicados y discursos de la Fed y del BCE | 06-2026 (Fed), 08-2026 (BCE) | **no hay historia**, ni noticias de empresas |
| Universo NASDAQ 100 (`cfds_intel.universe`, #10) | lista de **tickers** por fecha (sin sesgo de supervivencia) | — | **sin precios**: no hay ninguna acción en la base |

Para el tamaño del fuera de muestra se leyeron solo las fechas extremas, no precios ni resultados.

### 1.1 Eventos y noticias

**a) Acciones sueltas: deriva después de la noticia o sobrerreacción y reversión.**
- Fuentes: Chan, W. S. (2003). *Stock price reaction to news and no-news: drift and reversal after headlines*. Journal of Financial Economics, 70(2), 223–260 (con titulares públicos, las acciones siguen en la misma dirección durante semanas; sin noticia, el movimiento se revierte). Tetlock, P. C. (2007). *Giving content to investor sentiment: the role of media in the stock market*. Journal of Finance, 62(3), 1139–1168 (el pesimismo de los medios predice presión bajista que se revierte en días).
- Ejemplo del dueño (CEO de NVDA): es este tipo de evento. Para probarlo hace falta **un archivo de noticias con la hora de publicación real** por empresa, años de historia y precios de la acción. **No tenemos ninguna de las tres cosas** (ver inventario).
- **Resultados de empresas (PEAD):** Bernard, V. L. y Thomas, J. K. (1989). *Post-earnings-announcement drift: delayed price response or risk premium?* Journal of Accounting Research, 27 (suplemento), 1–36. Es de las anomalías más estudiadas, pero la evidencia reciente dice que en las acciones grandes casi desapareció: Martineau, C. (2022). *Rest in peace post-earnings announcement drift*. Critical Finance Review (volumen y páginas sin verificar). Las del NASDAQ 100 son justo las grandes.
- **¿Alcanza lo que tenemos para un estudio de eventos?** **No.** Falta, como mínimo:
  1. precios diarios (mejor intradía) de los componentes del NASDAQ 100, **incluidas las que salieron del índice**, 2013–2025. Ya se verificó que **no hay fuente gratuita** que deje guardarlos; la paga que mejor encaja es EODHD (`docs/research/precios-componentes-nasdaq100-2026-11.md`);
  2. fechas y horas de los anuncios de resultados (antes o después de la apertura);
  3. un archivo de noticias por empresa con hora de publicación real (proveedor pago, no verificado);
  4. los costos de IG para CFD de acciones: comisión, financiación y costo de préstamo para cortos (no están en `config/instruments.toml`).
- **Decisión del dueño:** operar acciones sueltas con CFD **cambia el universo del proyecto** (hoy: índices y oro). Antes de gastar en datos, el dueño decide si quiere ese cambio (con su ADR de proveedor de datos, `CLAUDE.md` §6).

**b) Eventos del calendario: la Fed, el CPI y el empleo.**
- Fuentes: Lucca, D. O. y Moench, E. (2015). *The pre-FOMC announcement drift*. Journal of Finance, 70(1), 329–371 (el S&P 500 sube en las 24 horas **antes** del anuncio de la Fed). Savor, P. y Wilson, M. (2013). *How much do investors care about macroeconomic risk? Evidence from scheduled economic announcements*. Journal of Financial and Quantitative Analysis, 48(2), 343–375 (los días de anuncios programados de inflación, empleo y Fed tienen retornos medios más altos).
- Datos: el calendario que tenemos (`economic_events`) empieza **esta semana**. Hace falta la historia 2011–2025 de las fechas y horas oficiales: FOMC (Reserva Federal), CPI y empleo (Bureau of Labor Statistics). Son datos públicos y chicos; cargarlos es un Issue de datos (ya propuesto como D2 en #466).
- Costo: muy bajo (pocas operaciones por año, R ancho). Es la familia **E** del ranking.

### 1.2 Horizontes largos: exposición manejada por volatilidad (familia V)

- Fuente: Moreira, A. y Muir, T. (2017). *Volatility-managed portfolios*. Journal of Finance, 72(4), 1611–1644. Escalan el factor por la **inversa de la varianza realizada del mes anterior** (`c / RV²`, con `c` elegido para que tenga la misma volatilidad que el factor sin manejar; según los autores, `c` no cambia el Sharpe). Para el mercado reportan alfas grandes contra el mercado sin manejar y que sobrevive a costos realistas; también una versión **sin apalancamiento**.
- En contra: Cederburg, S., O'Doherty, M. S., Wang, F. y Yan, X. S. (2020). *On the performance of volatility-managed portfolios*. Journal of Financial Economics, 138(1), 95–117: con reglas que se pueden aplicar en tiempo real (sin conocer `c` del período entero), la mejora fuera de muestra **no es robusta** en la mayoría de los factores. Por eso acá `c` se estima solo con la ventana de entrenamiento.
- **Por qué cambia la pregunta:** no intenta adivinar la dirección. Toma la prima del mercado (lo único que funcionó) y solo cambia **cuánto** se tiene. Una nota de nuestra ronda 2: la familia A ya dimensionaba por volatilidad (1 % de riesgo con stop a 3 ATR), pero además daba vuelta la dirección y cerraba cada viernes; V aísla la parte de la volatilidad.
- Datos: ES, NQ, YM desde 2011. **Se puede correr ya.**
- Costo: rebalanceo mensual (spread sobre lo que cambia la posición) y la financiación del CFD, igual que comprar y mantener.
- Límite importante: **cambia la spec** si se quisiera operar (tamaño por exposición en lugar de 1 % de riesgo, mantener el fin de semana y un stop de catástrofe). La prueba mide si vale la pena proponer ese cambio, no lo habilita.

### 1.3 Régimen de VIX y prima de riesgo de volatilidad

- Fuente: Carr, P. y Wu, L. (2009). *Variance risk premiums*. Review of Financial Studies, 22(3), 1311–1341 (la varianza implícita de las opciones es en promedio mayor que la realizada: quien vende seguro de volatilidad cobra una prima).
- Cosechar esa prima pide **opciones o futuros del VIX**, que no son nuestros instrumentos ni tenemos sus datos. Usar el VIX como **filtro de régimen** para los índices es posible, pero el VIX guardado empieza en 2019: con 24 meses de entrenamiento, la validación sería 2021–2025 (~19 ventanas), poca muestra para la regla de §3.
- La parte de "régimen" que sí se puede probar con 15 años es V (la volatilidad realizada del propio índice). Queda propuesta como **familia R**, después de cargar el VIX desde 2011 (Cboe publica la historia diaria; términos sin verificar).

### 1.4 Reducir costos: CFD de IG frente a micro futuros (MES) en un broker de futuros

No es una señal: es **cuánto cuesta tener la misma exposición**. Puede pesar más que cualquier regla.

| Concepto | CFD US500 de IG (de `config/instruments.toml`) | Micro E-mini S&P 500 (MES) |
|---|---|---|
| Spread en sesión | 0,9 puntos (medido en la demo, #67) | 1 tick, según la especificación de CME (**no verificado**: la página de CME no respondió a la lectura automática) |
| Spread fuera de sesión | 1,6–1,9 puntos (`docs/research/ig-spreads-2026-11.md`) | no verificado |
| Comisión | 0 (índices) | comisión del broker + tarifas de bolsa y regulatorias: **no verificado** (las páginas oficiales de IBKR devolvieron 403 a la lectura automática) |
| Financiación de un largo | referencia + **3 % anual** de tasa administrativa, sobre el nominal, cada noche (`ig-es-costes-indices`) | **sin tasa administrativa**: el costo de financiarse está dentro del precio del futuro (la base = tasa − dividendos) |
| Tamaño mínimo | 0,5 € por punto | 1 contrato (valor por punto a confirmar con CME) |

- **Lo que sí se puede decir sin números de IBKR:** para una posición que se mantiene (comprar y mantener, A, V), la tasa administrativa del CFD cuesta ≈ **3 % del nominal por año** más que el futuro, porque el futuro solo lleva la tasa del mercado (que el CFD también cobra). En la ronda 2, comprar y mantener el CFD dio ≈ +1,1 % por mes ya neto de esa financiación: un vehículo sin esa tasa le sumaría del orden de 0,25 % por mes al mismo riesgo. Es un cálculo aproximado, no un backtest.
- **Para intradía** la diferencia es el spread y la comisión, que falta verificar.
- **Lo que hace falta:** que una persona lea en el navegador las páginas oficiales de comisiones de IBKR (o del broker que se evalúe) y de especificaciones y tarifas de CME, y anote los valores con su fecha. Con eso se puede volver a calcular el costo de cada familia **sin** correr nada nuevo sobre los datos (no gasta presupuesto de error).
- **Decisión del dueño:** cambiar de broker o de tipo de producto requiere un ADR (`CLAUDE.md` §6) y, para dinero real, la aprobación de dos personas (§5). Abrir una cuenta no lo hace un agente.

### 1.5 Deriva nocturna en la apertura de Europa

- Fuente: Boyarchenko, N., Larsen, L. C. y Whelan, P. (2023). *The overnight drift*. Review of Financial Studies, 36(9), 3502–3547 (NY Fed Staff Report 917). Los retornos del S&P 500 se concentran entre las **2:00 y las 3:00 ET** (apertura de Europa), con un promedio de **3,7 % anualizado (1,48 puntos básicos por día)** según su introducción. Es más fuerte después de caídas del mercado.
- **Cuenta de costos, antes de mirar nada:** a esa hora (07–08 UTC en invierno) el spread de IG en US500 es **1,6 puntos** (`ig-spreads-2026-11.md`), más el slippage de la spec: ≈ 1,9 puntos por ida y vuelta, unos **3–4 puntos básicos** con el índice entre 5000 y 6500. **El costo es más del doble del efecto bruto publicado.** Con el CFD no puede pasar la regla, así que **no se gasta presupuesto en ella**.
- En la ronda 3 se mide **solo como descriptivo** (sin voto): el retorno bruto medio de 2:00–3:00 ET en nuestros futuros, para saber si el efecto existe en nuestros datos. Si existe, vuelve a ser candidata solo con un vehículo más barato (§1.4).
- La versión condicionada (después de caídas) sería otra familia: se deja anotada, sin correr.

### 1.6 Patrones de "manipulación" (informe #492, PR #494)

El informe de #492 concluye que los patrones ilegales (spoofing, wash trading, etc.) no se pueden ver con velas, y que los "comportamientos de mercado" (barridas, trampas, números redondos) tienen como mejor fuente a Osler, C. L. (2003). *Currency orders and exchange rate dynamics*. Journal of Finance, 58(5), 1791–1819, y Osler, C. L. (2005). *Stop-loss orders and price cascades in currency markets*. Journal of International Money and Finance, 24(2), 219–241: los stops se juntan **justo después** de los números redondos y de los máximos y mínimos, y cuando el precio los cruza, **acelera en la misma dirección** por unas horas. Propone dos familias:

- **M1. Cascada al cruzar un número redondo.** Continuación de 2 horas en la dirección del cruce de un múltiplo de `S` (US500 50, US100 100, US30 500), con los niveles intermedios `R + S/2` como control. Usa nuestras velas de 15m: **se puede correr ya**.
- **M2. Barrida frente a ruptura de los niveles del día anterior, a horas.** El propio informe pide hacerla primero como **estudio de eventos descriptivo** con lo guardado en `liquidity_level_events` y pre-registrarla solo si la diferencia supera el costo. Queda para después de ese descriptivo.
- **Vencimientos de opciones y marcar el cierre:** solo estudio descriptivo, sin operar (pedido del coordinador). No entran al ranking.

Riesgo de M1: la evidencia es en divisas, con datos de hace más de 20 años, y nuestro propio "sweep" de 15m perdió. M1 es distinta porque apuesta a la **continuación** (lo que dice Osler), no a la vuelta, y el horizonte es de horas.

## 2. Por qué la mayoría de las ventajas con noticias desaparecen para un minorista, y dónde sí hay evidencia de efectos lentos

- **Velocidad.** La reacción a una noticia programada (un dato de empleo, un anuncio de la Fed) se completa en segundos o menos. Busse, J. A. y Green, T. C. (2002). *Market efficiency in real time*. Journal of Financial Economics, 65(3), 415–437: con informes televisados de acciones, los precios se ajustan en minutos y operar después de un minuto ya no deja ganancia. Hoy, con lectura automática de noticias, es más rápido. Nuestro sistema decide al cierre de una vela de 15 minutos y ejecuta en la siguiente (`CLAUDE.md` §5b): llega **siempre** después.
- **Costos.** Aun si una noticia deja un movimiento que sigue, el CFD cobra spread (más ancho fuera de horario y en los minutos de los datos) y financiación. Un efecto de pocos puntos básicos no paga eso.
- **Publicación.** Las anomalías publicadas se achican después de publicarse: McLean, R. D. y Pontiff, J. (2016). *Does academic research destroy stock return predictability?* Journal of Finance, 71(1), 5–32. El PEAD en acciones grandes es el ejemplo (Martineau, 2022).
- **Dónde sí hay evidencia de efectos lentos** (horizontes de días o semanas, al alcance de alguien lento):
  - **Antes** del evento, no después: la deriva antes de la Fed (Lucca y Moench, 2015) y los días de anuncios programados (Savor y Wilson, 2013) no requieren leer la noticia: la fecha se conoce de antemano.
  - **Después de noticias de empresas**, deriva de semanas (Chan, 2003), sobre todo en acciones con menos seguimiento; en las grandes, menos.
  - **Manejo del riesgo**, no de la dirección: la volatilidad es mucho más predecible que el retorno; V se apoya en eso.
- **Conclusión práctica:** las ideas de noticias que tienen sentido para nosotros son las **programadas** (la fecha se sabe antes) o las de **horizonte largo**. Reaccionar a titulares en el momento no.

## 3. Presupuesto de error nuevo y regla de decisión

### 3.1 Presupuesto

- **Programa 2: 5 % en total**, igual que el programa de #466. Justificación: es el mismo nivel de exigencia; un programa nuevo no hace más probable que una familia sea real, así que no se afloja.
- **Ronda 3: 2,5 %**, Bonferroni entre las familias que entren (con 2 familias: **1,25 % unilateral por familia**).
- **Reserva: 2,5 %** para una ronda 4, para las familias que necesitan datos nuevos (E, M2 si su descriptivo lo justifica, R, o acciones si el dueño cambia el universo). Si la ronda 3 no deja nada, conviene usar la reserva en una familia con **otro tipo de dato** (eventos), no en variantes de lo mismo.
- **Sharpe deflactado con todas las pruebas registradas:** el programa nuevo **no** resetea el contador. Hasta la ronda 2 son 352; la ronda 3 suma las suyas (con 2 familias, N = 354). Lo que se probó antes sigue contando como búsqueda.
- **El fuera de muestra (desde el 01-11-2025) sigue intacto**: no se carga, no se mira, y se usaría una sola vez, para un solo candidato congelado, solo si el dueño lo aprueba.

### 3.2 Regla de decisión

Se reutiliza la de #466 §4 tal cual, con dos cambios justificados:

| Punto de #466 §4 | Programa 2 | Por qué |
|---|---|---|
| §4.1 walk-forward (24 + 3, paso 3), validación concatenada, costos de config, dos escenarios de financiación, estrés × 1,5 | **igual** | — |
| §4.2 una hipótesis por familia: cartera US500 + US100 + US30; US2000 descriptivo | **igual** | — |
| §4.3 criterio 1: retorno neto mensual de la cartera > 0, prueba t unilateral | **igual** para las familias que entran y salen (M1, E). Para **V**: la prueba es el **alfa contra comprar y mantener** (regresión de los retornos mensuales de V sobre los de comprar y mantener, con errores robustos), unilateral | V está casi siempre invertida: su retorno es positivo por la prima del mercado aunque el manejo no aporte nada. La hipótesis de Moreira y Muir es el alfa contra el factor sin manejar, y es lo que publican |
| §4.3 criterio 2: Sharpe deflactado ≥ 0,95 | **igual**; para **V** se deflacta la **ventaja sobre comprar y mantener** (alfa + residuo de la misma regresión) | por la misma razón: deflactar el Sharpe total de V premiaría la prima del mercado |
| §4.3 criterio 3: las dos mitades y 2 de 3 índices | **igual** (para V, el alfa en cada mitad y en cada índice) | — |
| §4.3 criterio 4: costos × 1,5 | **igual** (para V, el alfa con costos × 1,5) | — |
| §4.3 criterio 5: Sharpe ≥ comprar y mantener | **se exige a V** (como a A); para M1, un criterio propio: el R bruto medio de los cruces de números redondos > el de los niveles intermedios (el control de #492) | sin el control, M1 sería "cualquier ruptura", que ya se probó (C) |
| §4.4 qué se hace con el resultado | **igual**: pasa → al dueño; no pasa → se cierra y no se prueban variantes sobre estos datos | — |

Cada familia tiene su detalle operativo en el pre-registro de la ronda 3 (`docs/analysis/2026-11-ronda3-preregistro.md`), commiteado **antes** de correr.

## 4. Ranking

Criterios: evidencia publicada, costo frente al efecto esperado, datos que ya tenemos, y que sea una pregunta **distinta** de las cerradas.

| Orden | Familia | Evidencia | Costo frente al efecto | Datos | Ronda |
|---|---|---|---|---|---|
| 1 | **V. Exposición manejada por volatilidad** (índices) | fuerte (Moreira y Muir 2017), con una crítica seria de su versión en tiempo real (Cederburg et al. 2020) | bajo: rebalanceo mensual; la financiación es la misma que la de comprar y mantener | listos (2011–2025) | **3** |
| 2 | **M1. Cascada al cruzar un número redondo** | media: Osler 2003 y 2005, en divisas | medio: stop a 2 ATR de 15m, ≈ 0,07 R en US500; necesita un efecto de horas mayor que eso | listos (15m) | **3** |
| 3 | **E. Eventos programados: Fed, CPI, empleo** (antes del anuncio y el día del anuncio) | fuerte (Lucca y Moench 2015; Savor y Wilson 2013), pero con muestras anteriores a 2011 | bajo: pocas operaciones con R ancho (#466 estimó ≈ 0,03 R para la Fed) | **falta** el calendario histórico oficial (Issue de datos, público y chico) | 4 (reserva) |
| 4 | **M2. Barrida frente a ruptura del día anterior** | media (Osler 2005) | medio | listos, pero primero un **descriptivo** (lo pide #492) | 4, si el descriptivo lo justifica |
| — | R. Régimen de VIX | media | bajo | VIX solo desde 2019 | después de cargar el VIX desde 2011 |
| — | Acciones: noticias de empresas y PEAD | media; en acciones grandes, débil (Martineau 2022) | alto (CFD de acciones, cortos con préstamo) | **no hay**: precios, fechas de resultados, noticias con hora | solo si el dueño decide cambiar el universo |
| — | Deriva nocturna (2:00–3:00 ET) | fuerte (Boyarchenko et al. 2023) | **el costo del CFD es el doble del efecto bruto** | listos | solo descriptivo en la ronda 3 |
| — | Reducir costos (MES) | no es una hipótesis estadística | — | faltan las tarifas oficiales | lo verifica una persona; no gasta presupuesto |

**Ronda 3 = V y M1 (K = 2, 1,25 % cada una).** Más el descriptivo de la deriva nocturna, sin voto.

## 5. Qué decide el dueño

1. Si aprueba el programa 2 con su presupuesto (5 %: 2,5 % en la ronda 3 y 2,5 % de reserva) y la ronda 3 con V y M1. **Se pide la aprobación explícita en #493**; el agente no la da por hecha. Ver la nota de abajo.
2. Si se abre el Issue de datos del calendario histórico (FOMC, CPI, empleo, 2011–2025) para la familia E.
3. Si quiere estudiar acciones sueltas: es **cambiar el universo** del proyecto y pagar datos (ADR).
4. Que alguien lea las tarifas oficiales del broker de futuros y de CME (§1.4).
5. Si V pasara: que operarla pide un cambio de spec (tamaño por exposición, mantener el fin de semana, stop de catástrofe), que se aprueba aparte.

**Nota sobre el orden.** El pedido del dueño en #493 ("investiguemos ese cambio de pregunta… para ver como nos va") abre el programa. Siguiendo la práctica de las rondas 1 y 2, la ronda 3 se pre-registra y se corre en el mismo pedido, porque sus dos familias usan solo datos que ya tenemos y no cambian nada del sistema. Si el dueño no aprueba el presupuesto o las familias, los resultados de la ronda 3 se toman como **no válidos** (no como un resultado) y se vuelve a pre-registrar.

## Límites

- Varias citas de §1 y §2 (Chan, Tetlock, Bernard y Thomas, Martineau, Busse y Green, Savor y Wilson, Carr y Wu, Cederburg et al.) se dan como referencias publicadas; sus páginas no se pudieron leer de forma automática el 24-11-2026. Las de Moreira y Muir (2017) y Boyarchenko et al. (2023) se verificaron en los textos de NBER y de la Reserva Federal de Nueva York.
- Los costos de MES e IBKR **no se verificaron** y no se usan en ningún cálculo.
- Los efectos publicados suelen achicarse después de publicarse (McLean y Pontiff, 2016).
