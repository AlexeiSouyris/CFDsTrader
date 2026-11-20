# Por qué fallan la mayoría de los algoritmos de trading (y qué hacemos nosotros)

- **Issue:** #19
- **Fecha:** 2026-11-20
- **Propuesta que sale de acá:** [ADR 0003 – Modelo adaptativo y capas de contexto](../decisions/0003-modelo-adaptativo-y-contexto.md)

> Este documento no promete ganancias. Resume lo que está medido sobre por qué fallan las estrategias y compara cada causa con nuestra [spec de fase 1](../spec/fase-1-especificacion.md).

---

## 1. Lo que está medido

| Dato | Cifra | Fuente |
|---|---|---|
| Cuentas minoristas de CFDs que pierden dinero | ~70–80 % según el broker | Aviso obligatorio de ESMA/FCA que publica cada broker de la UE y el Reino Unido |
| Day traders rentables de forma sostenida, después de costos | menos del 1 % año a año | Barber, Lee, Liu y Odean, *Do Day Traders Rationally Learn About Their Ability?* (Taiwán) |
| Day traders de Brasil que insistieron más de 300 días | el 97 % perdió | Chague, De-Losso y Giovannetti (2019), *Day Trading for a Living?* |
| ¿El Sharpe del backtest predice el Sharpe real? | casi nada: R² < 0,025 en ~900 algoritmos | Wiecki et al. (2016), *All That Glitters Is Not Gold* (Quantopian) |
| Retorno de las anomalías publicadas | 26 % menor fuera de muestra y 58 % menor después de publicarse | McLean y Pontiff (2016), *Journal of Finance* 71(1) |
| Estrategias "descubiertas" probando muchas variantes | la mayoría son falsos positivos | Bailey, Borwein, López de Prado y Zhu, *The Probability of Backtest Overfitting*; Harvey, Liu y Zhu (2016), *…and the Cross-Section of Expected Returns* |

**Conclusión:** casi ningún algoritmo falla por un error de programación. Fallan porque **la ventaja medida nunca existió** (sobreajuste) o porque **se la comieron** los costos, los cambios de régimen o la competencia.

## 2. Patrones de error que se repiten

Estado en nuestra spec: ✅ cubierto · 🟡 parcial · ❌ falta. Lo que falta se propone en el [ADR 0003](../decisions/0003-modelo-adaptativo-y-contexto.md).

| # | Patrón de error | Cómo se ve | Nuestro estado | Qué falta |
|---|---|---|---|---|
| 1 | **Sobreajuste por pruebas múltiples** | Se prueban 500 variantes y se elige la mejor. Su backtest es el máximo de 500 tiradas con suerte | 🟡 walk-forward y datos fuera de muestra (§15) | No contamos **cuántas variantes se probaron** ni corregimos por eso. → registro de experimentos, Sharpe deflactado y PBO (ADR 0003, D) |
| 2 | **Look-ahead y fuga de información** | Usar el cierre de la vela que se está operando, o noticias antes de publicarse | ✅ decisión en `t` y ejecución en `t+1`, noticias por hora de publicación (CLAUDE.md §5b) | — |
| 3 | **Sesgo de supervivencia** | Probar solo con las empresas que hoy siguen en el índice | ✅ universo con empresas que salieron (#10) | — |
| 4 | **Costos subestimados** | En CFDs el spread y el swap overnight se comen las estrategias de poco margen | ✅ costos nunca en cero (§11) | Que los costos salgan del broker real (#7) antes de validar |
| 5 | **Cambio de régimen** | Una estrategia de tendencia gana en 2020–2021 y pierde en un mercado lateral. El backtest promedia los dos regímenes y esconde el problema | ❌ | → detector de régimen y resultados **por régimen** (ADR 0003, A y B) |
| 6 | **La ventaja se gasta** (*alpha decay*) | Un patrón conocido deja de funcionar cuando lo usa todo el mundo (McLean y Pontiff) | 🟡 el informe mensual mide el aporte por señal (§15b) | → monitor de deriva con pausa automática (ADR 0003, F) |
| 7 | **Riesgo concentrado sin darse cuenta** | 5 posiciones "distintas" en US Tech 100, US 500, Wall Street, DAX y Euro Stoxx son casi **una sola apuesta** con 5 % de riesgo | ❌ el límite es por posición y por instrumento (§10), no por correlación | → límite por grupo correlacionado (ADR 0003, E) |
| 8 | **Muy pocas operaciones** | 20 operaciones ganadoras no prueban nada: puede ser azar | ✅ mínimo de 30 casos para mostrar probabilidad y 50 operaciones en paper (§13, §15) | → además, comparar contra una **entrada aleatoria** con el mismo riesgo (ADR 0003, D) |
| 9 | **Apalancamiento y ruina** | Martingala, sin stop o con riesgo del 5–10 % por operación. Una mala racha liquida la cuenta | ✅ 1 % por operación, stop obligatorio, −3 % diario, −15 % de kill switch (§8, §10) | — |
| 10 | **Complejidad excesiva** | Cada señal nueva agrega parámetros y el modelo memoriza el ruido | 🟡 puntaje simple (§17) | Las capas de contexto nuevas **tienen que ganarse su lugar** fuera de muestra (ADR 0003, C) |
| 11 | **Sesgo narrativo** | "En Navidad la gente saca efectivo, entonces el mercado baja". Suena lógico, nadie lo prueba, y los datos dicen otra cosa (§3) | ❌ | → toda hipótesis entra como experimento con una predicción escrita **antes** de mirar los datos (ADR 0003, C) |
| 12 | **El humano rompe el sistema** | Apagarlo tras 3 pérdidas, duplicar el tamaño para recuperar, saltear el stop | ✅ el humano aprueba pero no edita el riesgo; reactivar requiere votación (§1, §1.2) | — |
| 13 | **Ejecución distinta del backtest** | Gaps, slippage, propuestas vencidas | ✅ gaps al precio de apertura, simulado contra real en el informe (§12, §15b) | — |
| 14 | **Datos malos** | Velas duplicadas, saltos falsos, horarios mal alineados | ✅ validación de velas (#5) y calendario (#9) | — |
| 15 | **Colas y crisis** | En un crash todas las correlaciones se van a 1 y los stops saltan por gaps | 🟡 stop-out y gaps simulados | → el límite de correlación (7) y un régimen de "no operar" en volatilidad extrema (ADR 0003, B) |

## 3. Las ideas del grupo, contrastadas con la evidencia

### 3.1 "En Navidad, Black Friday y Cyber Monday la gente saca efectivo y se invierte menos"
La evidencia apunta **al revés o es mixta**:
- El llamado *Santa Claus rally* (últimos 5 días hábiles de diciembre y los 2 primeros de enero) muestra en promedio retornos **algo positivos**, no negativos. Un estudio del *Journal of Financial Planning* (2015) encuentra retornos diarios más altos en ese período en EE. UU. y en muchos otros mercados.
- Otros análisis no encuentran un efecto confiable una vez que se corrige por el "efecto enero", y el resultado cambia mucho según cómo se defina la ventana.
- El gasto de Navidad de los hogares es chico frente al volumen del mercado, que lo mueven sobre todo los institucionales. Además, es un patrón **conocido por todos**, así que ya está en el precio.

**Qué hacemos:** el calendario entra como **contexto medible** (feriados, días de bajo volumen, vencimientos de opciones, cierre de trimestre, FOMC), no como regla. La hipótesis se registra antes de mirar los datos y se prueba fuera de muestra.

### 3.2 IPOs, como la de Anthropic
Anthropic empezó a cotizar en el Nasdaq como **ANT** el 19-11-2026, a 83 USD por acción según las fuentes de la búsqueda. Lo que dice la evidencia sobre las IPOs en general:
- **Fin del lock-up** (cuando los empleados y los inversores previos pueden vender, normalmente a los 90–180 días): en promedio, un retorno anormal de **−1,5 % en 3 días** y un 40 % más de volumen (Field y Hanka, 2001). Es un evento con **fecha conocida**: sirve como contexto objetivo.
- **Largo plazo:** las IPOs suelen rendir menos que comparables durante los 3–5 años siguientes (Ritter, 1991, y trabajos posteriores).
- **Efecto en el sector:** una IPO gigante puede sacar dinero de otras acciones del mismo sector, porque los fondos venden otras posiciones para comprar la nueva. Para nosotros es una **hipótesis** para medir sobre los componentes de IA del NASDAQ 100, no un hecho.

**Límites:** ANT no está en nuestra lista de instrumentos (§2), no sabemos si el broker ofrecerá su CFD, y una acción con días de historia no tiene datos suficientes (regla de mínimo 30 casos). En v1 entra **solo como evento de contexto**: IPO, fin del lock-up y entrada a índices.

### 3.3 Miedo, emociones, protestas y política
Tiene sentido como **contexto**, y parte ya está en la spec (VIX como contexto §13, sentimiento de noticias por LLM §9). Lo que falta:
- **Medidores de miedo** objetivos: VIX y su estructura, put/call, encuestas de sentimiento. Cada fuente se agrega con una vía de acceso legal y documentada en un ADR, como ya exige §9.
- **Eventos políticos y sociales** (protestas, elecciones, sanciones, aranceles): el LLM los clasifica como un evento con tipo, país, sector afectado, dirección e intensidad, **con su hora de publicación real**.
- Riesgo principal: estos eventos son **raros y distintos entre sí**, entonces hay pocos casos para medir. Por eso se muestran con su incertidumbre y empiezan como **experimentales**.

### 3.4 "Que el modelo sea cambiante y que no se le pueda estudiar un patrón"
Hay dos ideas distintas mezcladas:
1. **Que se adapte al mercado: sí.** Ese es justo el error n.º 5 de la tabla. Se propone un detector de régimen que cambia de **juego de parámetros** según el contexto (ADR 0003, A y B).
2. **Que sea impredecible o aleatorio para que nadie lo copie: no.** Con un fondo de 1.000 € nadie va a estudiar nuestras órdenes: somos invisibles para el mercado. Y meter azar en las decisiones:
   - no agrega ventaja, solo ruido;
   - hace imposible medir si el sistema funciona, porque el mismo backtest daría resultados distintos cada vez;
   - rompe reglas nuestras: funciones puras y misma entrada → misma salida (CLAUDE.md §5b).

   La forma correcta de "no ser predecible" es **mantener la estrategia privada**, y eso ya pasa.

## 4. Resumen

Las causas de fracaso más comunes que **ya cubrimos** son look-ahead, supervivencia, costos, apalancamiento, datos malos y el humano rompiendo reglas. Las que **faltan** son:
1. Corrección por pruebas múltiples (registro de experimentos, Sharpe deflactado y PBO).
2. Adaptación al régimen de mercado.
3. Límite de riesgo por grupo de instrumentos correlacionados.
4. Monitor de deriva con pausa automática.
5. Un circuito para que las ideas nuevas (sentimiento, política, calendario, IPOs) entren sin sobreajustar: como experimento primero, y como señal solo si demuestran valor fuera de muestra.

Todo eso se propone en el [ADR 0003](../decisions/0003-modelo-adaptativo-y-contexto.md).

## Fuentes

- ESMA, medidas de intervención sobre CFDs y aviso de riesgo obligatorio (2018, permanentes desde 2019).
- Barber, B., Lee, Y., Liu, Y., Odean, T. *Do Day Traders Rationally Learn About Their Ability?*
- Chague, F., De-Losso, R., Giovannetti, B. (2019). *Day Trading for a Living?*
- Wiecki, T. et al. (2016). *All That Glitters Is Not Gold: Comparing Backtest and Out-of-Sample Performance on a Large Cohort of Trading Algorithms.*
- [McLean, R. D., Pontiff, J. (2016). *Does Academic Research Destroy Stock Return Predictability?* Journal of Finance 71(1), 5–32](https://econpapers.repec.org/RePEc:bla:jfinan:v:71:y:2016:i:1:p:5-32)
- Bailey, D., Borwein, J., López de Prado, M., Zhu, Q. *The Probability of Backtest Overfitting.*
- Bailey, D., López de Prado, M. (2014). *The Deflated Sharpe Ratio.*
- Harvey, C., Liu, Y., Zhu, H. (2016). *…and the Cross-Section of Expected Returns.*
- [Field, L., Hanka, G. (2001). *The Expiration of IPO Share Lockups.* Journal of Finance 56(2)](https://ideas.repec.org/a/bla/jfinan/v56y2001i2p471-500.html)
- Ritter, J. (1991). *The Long-Run Performance of Initial Public Offerings.* Journal of Finance 46(1).
- [*Yes, Virginia, There Is a Santa Claus Rally*, Journal of Financial Planning (2015)](https://financialplanningassociation.org/article/journal/MAR15-yes-virginia-there-santa-claus-rally-statistical-evidence-supports-higher-returns-globally)
- [Interactive Brokers Campus, análisis del Santa Claus rally](https://www.interactivebrokers.com/campus/?p=216345)
- IPO de Anthropic: [Wikipedia](https://en.wikipedia.org/wiki/Initial_public_offering_of_Anthropic), [StockTitan](https://www.stocktitan.net/articles/anthropic-ipo-live-ant-first-trading-day-2026), [Bloomberg](https://www.bloomberg.com/news/articles/2026-10-01/anthropic-said-to-target-mega-ipo-before-thanksgiving-holiday). Los montos de valuación y lo recaudado difieren entre fuentes; verificar en el prospecto final (SEC).
