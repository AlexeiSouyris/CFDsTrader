# Gianluca Bruni: quién es, qué estrategias explica y qué podemos aprender

- **Issue:** #561 (programa 4). **Escrito:** 25-11-2026 por un agente de IA (Claude Code) para el dueño.
- **Pedido del dueño** (#561): investigar en profundidad sus estrategias con buenas fuentes, «aprender qué dice y qué hace diferente a todos».
- **Cómo se hizo:** se leyeron la página oficial de la competencia y artículos de terceros (solo lectura web), y las transcripciones automáticas de 6 videos de su canal y de 4 entrevistas. Todas las fuentes están en §9, con link y fecha.
- **Antes, en `main`:** el análisis del video en vivo contra Emre (`docs/research/videos/2026-11-25-emre-vs-gianluca.md`). Este informe lo amplía y no lo repite.
- La prueba con nuestros datos está en el borrador de pre-registro `docs/analysis/2026-11-programa-4-preregistro.md`.

Nada de esto es una recomendación de operar. Lo que él muestra son resultados pasados de otra persona, en otro mercado y con otro riesgo, y no prometen nada.

## 1. Resumen en cinco líneas

1. **Ganó de verdad** el campeonato **trimestral** de forex de la World Cup Trading Championships tres veces (2025 Q4, 2026 Q2 y 2026 Q3), con dinero real. El de 2026 Q4 lo va ganando, pero **todavía no terminó**. No ganó el campeonato **anual**.
2. Lo que lo diferencia no es un indicador secreto, sino el **proceso**: reglas objetivas (sin "se ve como"), muchas operaciones antes de confiar, costos reales, prueba fuera de muestra y compararse contra comprar y mantener.
3. Según él, la mitad de su ganancia en competencia viene del **tamaño variable**: de 0,5 % a 4 % por operación con una fracción de Kelly. Eso choca con nuestras reglas de riesgo y **no lo adoptamos**.
4. Tiene **conflictos de interés claros**: empresas de fondeo (prop firms) que lo patrocinan, un código de descuento a su nombre y cursos pagos en el pasado. Además, sus relatos tienen inconsistencias.
5. Vale la pena **probar tres reglas suyas** con nuestra metodología completa en el US100: la de prop firms (A, literal y normalizada) y la "de campeonato" (C). Lo más probable es que no pasen.

## 2. Ficha verificada

### 2.1 La competencia

- **Qué es:** el **Quarterly Forex Trading Championship** de los World Cup Trading Championships (WCC), conocido como "Robbins Cup". Lo organiza el grupo Robbins, que corre competencias desde 1983. No es el campeonato anual "World Cup Forex", sino las ediciones de tres meses.
- **Reglas que se pudieron leer:**
  - se opera una **cuenta propia con dinero real** en un broker, que confirma los resultados;
  - el ranking es por **% neto** después de comisiones;
  - el mínimo para la trimestral es de **USD 2.500**, y para el trofeo hacen falta 10 operaciones completas (ida y vuelta);
  - el apalancamiento llega a **200:1** en forex (solo para cuentas no estadounidenses);
  - algunas posiciones figuran como "Final Pending Audit", o sea que hay una auditoría antes de cerrar.
- **Reglas que no se pudieron leer:** los contratos de inscripción están en robbinstrading.com y no se pudieron abrir. Por eso **no sabemos** cómo se tratan los depósitos y retiros, si se permiten varias cuentas por persona, cómo se verifica todo ni cuántos compiten.

### 2.2 Resultados oficiales

Fuente: tabla oficial de posiciones de la WCC, leída el 25-11-2026 (§9). Él figura con país "UAE".

| Trimestre | Puesto | % neto | Estado |
|---|---|---|---|
| 2025 Q3 | 4.º | 71,6 % | Final |
| 2025 Q4 | **1.º** | 58,6 % | Final |
| 2026 Q1 | no figura entre los 5 primeros (el 5.º hizo 9,3 %) | — | Final |
| 2026 Q2 | **1.º** | 104,8 % | Final |
| 2026 Q3 | **1.º** | 245,2 % | Final |
| 2026 Q4 | 1.º **provisorio** | 205,1 % | En curso: datos al 23-11, cierra el 31-12 |

**Sobre lo que se dice de él:**

- "Ganó Q2, Q3 y Q4 de 2026": Q2 y Q3 son **ciertos** (resultados finales). Q4 **todavía no**: va primero, pero falta más de un mes.
- Suma **tres títulos finales** (2025 Q4, 2026 Q2 y 2026 Q3), no dos.
- El **anual** de forex 2026 lo lidera otra persona (314,1 %).
- Chart Fanatics lo presenta como campeón de **futuros**. Es falso: compite en forex.

### 2.3 Lo que no se pudo verificar

- Lo que dice haber cobrado de empresas de fondeo: más de USD 1 M gestionado en 6 firmas, USD 66.000 en un ciclo, más de 200.000 o 300.000 USD en pagos (da las dos cifras en videos distintos).
- Sus backtests: "15 años y más de 5.000 operaciones", 147 R en 2 años, Sharpe 2,2. Son capturas de pantalla, no hay un informe auditado.
- Con cuánto capital compite, cuántas operaciones hizo, su peor caída real y si usó varias cuentas o reinició alguna.

### 2.4 Quién es (según él y según entrevistas)

Dice que es siciliano, que empezó a operar en 2020 a los 16 años, que dejó la universidad en Bolonia y que vive en Dubái. Antes enseñaba en Morpheus Education, una escuela italiana de trading. En 2024 una empresa de fondeo ya lo presentaba como trader de forex fondeado en Dubái.

## 3. Sus estrategias

Notación: **[dice]** = lo que él explica en video (fuente oral, transcripción automática, habla un inglés con errores). **[inferido]** = lo interpretamos nosotros porque él no lo dejó claro.

### 3.A Estrategia "para prop firms" (video en vivo del 30-10-2026)

Ya está explicada en detalle en `docs/research/videos/2026-11-25-emre-vs-gianluca.md` §2. En corto:

- **[dice]** Disparador: **dos velas seguidas a favor**, cada una con un cuerpo de **al menos 60 ticks (15 puntos) en el futuro del Nasdaq (NQ)**, en velas de **15 minutos**. Eligió 60 como un valor que "cuadra" con su backtest, no como el óptimo, para no sobreajustar.
- **[dice]** Solo a favor de la estructura, con la **actual** y la **periférica** alineadas (§3.B).
- **[dice]** Stop en el extremo de la primera vela; objetivo fijo de **1,5 veces el riesgo**.
- **[dice]** Piramida: si sale otra vela válida, suma tamaño y mueve el stop.
- **[dice]** Expectativa de **0,12 R** por operación. Él mismo dice que **no es su estrategia principal**, que está pensada para empresas de fondeo y que con plata propia probablemente no le gana a comprar y mantener el índice.

### 3.B Cómo lee la estructura (común a todas)

Es la parte más útil, porque convierte el "análisis técnico" en reglas que se pueden medir:

- **[dice]** **Vela válida:** el cuerpo es más grande que la mecha más larga.
- **[dice]** **Zona** (consolidación): un tramo con al menos **3 velas válidas del color contrario** al lado de la operación.
- **[dice]** **Ruptura:** cuenta solo si el **cuerpo cierra** fuera de la zona, nunca una mecha. Habla de "un mínimo de distancia", pero no da el número.
- **[dice]** **Estructura actual y periférica:** mira las dos últimas rupturas. La más reciente es la **actual** y la anterior la **periférica**. Sigue la actual mientras no vaya contra la periférica.
- **[dice]** El límite del movimiento es el nivel del 75 % del tramo, o el **área de valor** del perfil de volumen, que considera más precisa. Al llegar ahí, manda la periférica.
- **[dice]** Usa **un solo marco de tiempo**: analiza y entra en 15 minutos.
- **[dice]** Su versión vieja ("prop killer") operaba la manipulación del rango asiático en la apertura de Londres: entrada con una vela envolvente (engulfing) de 15 minutos, stop detrás de la manipulación, objetivo en el 50 % del rango asiático o del otro lado, R:R de 1:3 a 1:5. Le agregó un filtro de distancia entre mechas (≥ 0,6, calibrado en 15 años con datos de un broker de forex). Él mismo avisa que ese umbral no se traslada a otro broker.
- **[dice]** Antes de la competencia recalculó todo con los costos del broker del torneo (spread, comisión, financiación, slippage) y **descartó cerca del 30 % de los activos**.

### 3.C Estrategia "de campeonato" (entrevista IQCapital del 02-10-2026)

- **[dice]** **Régimen de volatilidad** con el percentil del ATR: ATR de 14 velas comparado con las últimas 200. Cuatro tramos: 0–25 bajo, 26–50 medio, 51–75 alto, 76–100 extremo. Su estrategia rinde mejor en el **alto**.
- **[dice]** **Frescura del régimen** con un indicador de TradingView de detección de cambios (bayesiano), en escala 0–100. Prefiere el régimen "nuevo". **[inferido]** No es reproducible con una definición pública, así que no lo usamos.
- **[dice]** **Perfil de volumen de cada sesión.** Si el área de valor (la franja de precios donde se operó el 70 % del volumen) se **desplaza** de una sesión a la siguiente, hay tendencia.
- **[dice]** **Setup largo:** el precio vuelve al área de valor de la **sesión anterior** sin romper el mínimo de la estructura; entra cuando una vela **cierra por encima del borde superior del área de valor (VAH)** y por encima de la mecha más alta de la vela anterior, con un margen mínimo (dice **6 puntos en NQ**, 0,6 pips en forex).
- **[dice]** **"Eficiencia":** la vela de entrada tiene que tener menos volumen que la anterior, o menos que su media de 20 velas.
- **[dice]** Stop en el mínimo de la vela de entrada y objetivo fijo de **2 veces el riesgo**. Dice que probó de 1:1 a 1:5 y que 1:2 dio la mejor expectativa. Puede reentrar con la misma condición. No opera días de datos fuertes (empleo, inflación) porque, según él, rinden mal. Es mejor en la sesión de Nueva York que en la de Londres.
- **[dice]** Números que muestra: expectativa de 0,2 R, Sharpe 2,2, 147 R en 2 años, 41 de 10.000 caminos de Monte Carlo en pérdida.

### 3.D Modelo de riesgo (según él, explica "la mitad" de lo que ganó en la competencia)

- **[dice]** **Autocorrelación de resultados:** compara la tasa de aciertos **después de** una ganadora o una perdedora con la tasa general. Dice que en condiciones normales las pérdidas siguen a pérdidas, así que **saltea la operación que sigue a una pérdida**. Revalida cada unas 100 operaciones y en ventanas móviles de 15 años, y si la relación no es estable la descarta.
- **[dice]** En las rachas muy largas (8 pérdidas seguidas) dice que la relación se da vuelta y que la siguiente operación tiene más chances de ganar.
- **[dice]** **"Variance accelerator" (VAM):** sube el riesgo de 0,5 % hasta 4 % por operación cuando la racha perdedora llega a su promedio o a su máximo histórico, y también a medida que crece la cuenta. En otra entrevista da 0,5 % a 3 %.
- **[dice]** **"Meta-tagging":** un puntaje de 0 a 5 (régimen, posición en la racha, frescura, volumen, sesión de Nueva York). Con 5/5 arriesga ½ Kelly, con 4/5 ¼ Kelly y con menos, fracciones más chicas.
- **[dice]** Para empresas de fondeo: 4 cuentas de 50.000 en vez de una de 200.000, operando una por semana ("drawdown shield"). En el régimen "calmo" (medido con GARCH) dice que su estrategia pierde el 75 % de las veces y que ahí la **invierte**.

### 3.E Cómo valida

- **[dice]** Mira la **expectativa en R**, no la tasa de aciertos.
- **[dice]** **Incertidumbre ≈ 1/√N:** con 10 operaciones, ±31 %; con 1.000, ±3 %. Pide al menos unas 100 operaciones antes de concluir. Su "ruido" por operación y la cantidad de operaciones necesarias, (ruido / expectativa)², son correctos: con 55 % de aciertos a 1:1 salen unas 100.
- **[dice]** Separa dentro y fuera de muestra y no cambia reglas fuera de muestra. Tolera hasta 20–25 % de degradación; más del 50 % invalida la estrategia.
- **[dice]** Cuenta cuántas variantes probó (probabilidad de sobreajuste), "rompe" la estrategia a propósito (mueve entrada y stop, empeora la ejecución, la prueba en otro activo), usa repetición vela a vela para no mirar el futuro y Monte Carlo para conocer la peor racha.
- **[dice]** Si no le gana a comprar y mantener, no la opera con cuenta propia.
- **[dice]** Dice ser "95 % mecánico": deja una capa chica de juicio humano para recalibrar.

## 4. Qué hace diferente

Comparado con casi todo el contenido de trading en video:

1. **Definiciones medibles.** Vela válida, zona y ruptura son reglas que una computadora puede aplicar sin interpretar. Es raro en el trading "de estructura".
2. **Estadística básica bien usada.** Su 1/√N, el ruido por operación y el tamaño de muestra necesario están bien (ver la verificación numérica en el análisis del video contra Emre, §3).
3. **Costos antes que nada.** Recalibró con los costos del broker del torneo y descartó activos que dejaron de funcionar.
4. **Compararse con comprar y mantener.** Lo dice explícitamente y admite que su estrategia de prop firms no le gana.
5. **El tamaño como parte de la estrategia.** Esto es lo más distinto y lo más peligroso (§5 y §7).

## 5. Alertas y conflictos de interés

1. **Empresas de fondeo.** Las dos entrevistas de IQCapital son contenido de una empresa de fondeo, que vende desafíos desde USD 9. La del 02-10-2026 dice: "Use code 'GB' for the maximum discount". **[inferido]** Es muy probable que sea un código de afiliado suyo. Otra entrevista es un testimonio para Ufunded, y Chart Fanatics viene con publicidad de Apex y NinjaTrader. Regala desafíos en su Telegram. Gana más si más gente compra desafíos.
2. **Cursos.** Fue formador de un curso pago de Morpheus Education (21 semanas, en italiano), que prometía cuentas fondeadas de hasta USD 300.000. La única fuente es un revendedor; no se encontró la página oficial ni el precio. Dice que dejó de enseñar para competir.
3. **La competencia premia el riesgo.** Gana el mayor % en tres meses, sin límite de riesgo ni de caída. Eso favorece justamente arriesgar 4 % por operación y medio Kelly. Una estrategia que gana un torneo así puede ser mala para una cuenta que tiene que durar años.
4. **Sesgo de supervivencia y campo chico.** Se pueden usar varias cuentas, los intentos fallidos no se ven y solo se publica el top. En 2026 Q2 el 3.º al 5.º hicieron entre 2,7 % y 3,3 %, lo que sugiere pocos competidores. Ganar varias veces es más difícil de explicar por suerte, pero no sabemos cuántas cuentas compiten.
5. **Falacia del jugador contra autocorrelación.** Dice dos cosas que tiran para lados opuestos: que las pérdidas siguen a pérdidas (por eso saltea la próxima) y que después de 8 pérdidas la siguiente tiene más chances de ganar (por eso sube el riesgo). Lo segundo es la **falacia del jugador** salvo que se demuestre con muchos datos. Con 8 pérdidas seguidas hay muy pocos casos para medir eso, y subir el riesgo justo ahí es lo que más daño hace si se equivoca.
6. **Relatos inconsistentes:**
   - en su biografía llama "primer y segundo trimestre" a los de 2025 Q3 y Q4;
   - da 71 % y 76 % para el mismo trimestre;
   - da 0,25 % y 0,5 % de riesgo para la misma estrategia;
   - en febrero de 2026 dijo ir "72–73 %" en el trimestre en curso, pero en 2026 Q1 terminó fuera del top 5;
   - los entrevistadores inflan ("futures champion", "3 veces campeón" antes de tiempo).
7. **Ningún backtest auditado.** Todas sus métricas son capturas de pantalla. Él mismo dice que no hay que confiar en nadie y que hay que probarlo.
8. **En vivo no siguió su regla.** En el video en vivo bajó a 5 minutos y tomó una operación que él llamó "no el mejor setup" porque había que operar.

## 6. Qué aprendemos

**Lo que ya hacemos y él confirma:** reglas objetivas, cientos de operaciones antes de concluir, costos completos, fuera de muestra intocable y comparación contra comprar y mantener (`CLAUDE.md` §5b; criterios de los programas anteriores).

**Ideas nuevas que vale la pena probar** (en el programa 4, con presupuesto propio):

1. **Su estructura objetiva** (vela válida, zona con 3 velas válidas del color contrario, ruptura con cierre de cuerpo, actual y periférica) como filtro de dirección. Ya escrita como definición medible en el borrador.
2. **La estrategia A** en US100, literal (15 puntos) y normalizada por volatilidad.
3. **La estrategia C** (perfil de volumen de sesión, régimen por percentil del ATR, cierre sobre el VAH, eficiencia de volumen, 1:2), con el volumen real del NQ.
4. **El test de suerte** (entradas al azar con el mismo stop, objetivo y cantidad) como reporte fácil de entender, al lado del p-valor y el Sharpe deflactado.
5. **"Saltear tras una pérdida"**, solo si la autocorrelación de resultados resulta estable en ventanas móviles. Queda como candidata para la reserva del programa.

## 7. Qué no adoptamos

- **El VAM y el Kelly variable** (0,5 % a 4 % por operación, ½ Kelly): chocan con el riesgo del proyecto, que es **1 % por operación, pérdida máxima diaria del 3 % y caída máxima del 15 %** (`config/risk.toml`, `CLAUDE.md` §5). Con 4 % por operación, una racha de 8 pérdidas, que él dice haber tenido, se lleva más del 25 % de la cuenta. Cambiarlo pediría un cambio de spec, y no hay evidencia que lo justifique.
- **Subir el riesgo después de rachas perdedoras:** por la falacia del jugador (§5.5).
- **La piramidación:** sumar tamaño cambia el riesgo de la operación después de entrar. Habría que definirla con números y pasarla por el módulo de riesgo. Queda fuera del programa 4.
- **La "frescura" bayesiana:** es un indicador de TradingView sin definición pública que podamos reproducir.
- **Invertir la estrategia en el régimen "calmo":** es otra hipótesis sin datos, y armar reglas después de ver cuándo pierde es sobreajuste.
- **El filtro de distancia entre mechas (≥ 0,6):** él mismo dice que depende del broker de datos, y no está definido para el US100.
- **Las tácticas de prop firms** (varias cuentas, "drawdown shield"): no aplican a nuestra cuenta.

## 8. Conclusión

Es buen material **de proceso**: muestra cómo convertir ideas de trading en reglas medibles y cómo pensar en muestras grandes. Sus títulos trimestrales son reales. Pero no hay evidencia auditada de una ventaja, tiene incentivos comerciales para mostrarse ganador, y la mitad de lo que gana en el torneo, según él, viene de un manejo del riesgo que nosotros no queremos. Probamos sus reglas de entrada con nuestra metodología y nuestro riesgo. Si no pasan, se cierran sin probar variantes.

## 9. Fuentes

Todas leídas o transcriptas entre el 24 y el 25-11-2026. Las transcripciones son automáticas (subtítulos de YouTube en inglés). La entrevista IQCapital del 02-10-2026 tiene la pista de audio doblada; sus detalles quedan **a confirmar** con una transcripción completa (punto abierto del pre-registro).

**Oficiales y terceros**

- World Cup Trading Championships, posiciones: https://www.worldcupchampionships.com/world-cup-trading-championship-standings (leído el 25-11-2026). Fuente clave de §2.2.
- World Cup Trading Championships, formatos y mínimos: https://www.worldcupchampionships.com/events (25-11-2026).
- World Cup Trading Championships, índice de reglas: https://www.worldcupchampionships.com/rules (25-11-2026). Los contratos de inscripción no se pudieron abrir.
- Kagels Trading, resumen y críticas de la competencia: https://kagels-trading.com/blog/world-cup-trading-championship/ (25-11-2026).
- Revendedor del curso de Morpheus Education (solo para la descripción del curso): https://coursesmint.com/product/morpheus-education-hybrid-trading-accelerator-pro/ (25-11-2026).
- Promoción de terceros ("2x champion"): https://x.com/chartfanatics/status/2103140829275607503 y https://x.com/Wordsofrizdom/status/2093722771998032076 (25-11-2026).

**Entrevistas en video**

- IQCapital, en vivo contra Imre Gams, 30-10-2026: https://www.youtube.com/watch?v=2WZ137MtCiw (estrategia A, 1/√N, 0,12 R, piramidación).
- IQCapital, 02-10-2026, 1 h 43 min: https://www.youtube.com/watch?v=nlLGlVJGrtw (estrategia C, régimen, meta-tagging, Kelly, código "GB").
- Chart Fanatics, 30-08-2026, 2,7 h: https://www.youtube.com/watch?v=xl4QHqlBCfk (actual y periférica, 33 activos, Kelly, Monte Carlo, drawdown shield, GARCH).
- Ufunded, 26-02-2026: https://www.youtube.com/watch?v=Bva2DnH1bgk (testimonio, "72–73 %" del trimestre en curso).
- FundedNext, 24-08-2024: https://www.youtube.com/watch?v=mJPMj7suzgg (trader fondeado en Dubái, pares que opera).
- No se pudieron transcribir (límite de YouTube): World Class Edge, 15-10-2026 (https://www.youtube.com/watch?v=b7KxHemoAp0), y Wayond, 26-06-2026 (https://www.youtube.com/watch?v=bTBI6mUz7PQ).

**Su canal (@gianlucabrunii)**, transcriptos el 25-11-2026:

- "I Quit College and became World Trading Champion at 22", 07-07-2026: https://www.youtube.com/watch?v=5m18-7dggXE (biografía, prop killer, VAM de 0,5 % a 4 %).
- "The Risk Model That Made Me World Trading Champion": https://www.youtube.com/watch?v=RpNBLyqJaZg (autocorrelación, saltear tras una pérdida, VAM, Kelly condicional).
- "Before I Trust Any Trading Strategy, I Check These 2 Numbers": https://www.youtube.com/watch?v=hseaM-MjsSk (varianza, incertidumbre, tamaño de muestra).
- "Stop Trying To Predict the Market (Do This Instead)": https://www.youtube.com/watch?v=WjZPrshYxIM (método en 5 pasos).
- "I Don't Care How Liquidity Works. I just profit from it": https://www.youtube.com/watch?v=W8s7i0Q8W1g (filtro de distancia entre mechas).
- "I Took A 17-Minute Cold Shower To Fix My Trading": https://www.youtube.com/watch?v=MNyhupN6n2w (psicología; poco útil).
- "Before You Buy Another Prop Firm Challenge…": https://www.youtube.com/watch?v=d144Xr6Y6v0 (sin subtítulos; no se analizó).
