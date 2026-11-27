# Bruni en vivo contra Imre Gams: qué muestra en los gráficos

- **Issue:** #853. **Escrito:** 26-11-2026 por un agente de IA (Claude Code) para el dueño.
- **Video:** «IQCapital en vivo contra Imre Gams», publicado el 30-10-2026, 55 min: https://www.youtube.com/watch?v=2WZ137MtCiw
- **Cómo se hizo:** se miraron fotogramas del video (uno cada 15 segundos más los cambios de escena, y algunos a tamaño completo para leer números) y se cruzaron con los subtítulos automáticos en inglés. Las imágenes **no están en el repo** (derechos de autor). Quedan solo en la máquina del dueño, fuera del repo.
- **Antes, en `main`:** los apuntes del mismo video hechos solo con los subtítulos (`docs/research/videos/2026-11-25-emre-vs-gianluca.md`) y el informe general (`docs/research/2026-11-gianluca-bruni.md`, §3.A). Este documento agrega **lo que se ve** y corrige lo que los subtítulos solos no dejaban ver.

**Marcas:** **[dice]** = lo dice en voz alta. **[muestra]** = se lee en la pantalla. **[inferido]** = lo deducimos nosotros, puede estar mal.

Nada de esto es una recomendación de operar. Es una sesión de unas 3 horas de otra persona, con su cuenta y su riesgo, y no prueba nada sobre el futuro.

## 1. Resumen en diez líneas

1. **No usa TradingView para operar.** Usa otra plataforma (en italiano, sobre Windows), con el contrato **micro** del Nasdaq (**MNQ**, vencimiento diciembre de 2026), no el NQ grande [muestra].
2. Lo único que tiene en el gráfico es el **perfil de volumen de cada sesión**, con el área de valor, el POC y dos o tres líneas horizontales y rectángulos que dibuja a mano [muestra].
3. "**60 ticks**" son **15 puntos**: la regla de la plataforma muestra los dos números juntos, por ejemplo «15,50 (62 ticks)» y «9,75 (39 ticks)» [muestra].
4. **Hizo dos operaciones, no tres:** un corto que tocó el stop (unos −940 USD) y un largo con **una sola suma** de contratos que llegó al objetivo (unos +3.000 USD). Termina el día en **+2.038 USD** [muestra].
5. El objetivo **no queda exactamente en 1,5 R**: en las dos operaciones queda entre 1,6 y 1,7 veces el stop. Parece que fija el objetivo **en dólares** (1.500 USD sobre un riesgo pensado de 1.000) [muestra + inferido].
6. Al sumar contratos, el **riesgo en dólares se duplicó** (de 940 a 1.914 USD) y después **movió el objetivo** para que diera 3.000 USD [muestra].
7. El largo lo tomó **contra su propia regla**: la estructura actual era alcista y la periférica bajista. Él mismo dice que «no es el mejor setup» [dice + muestra].
8. Su backtest se ve en una web llamada **Caliber**, con la estrategia «AXIOM NQ»: **405 operaciones del 7-1 al 30-9-2026 (nueve meses)**, +0,12 R por operación, 44,9 % de aciertos, t = 2,00 [muestra].
9. Esas cifras **cierran entre sí** (las recalculamos), pero son **un solo período corto**, sin prueba fuera de muestra a la vista, y en la misma web hay otras estrategias guardadas. Con t = 2,00 justo, probar varias variantes alcanza para que una salga así por azar.
10. Nosotros ya probamos esta estrategia (programa 4, H4.1 y H4.2, filas R029 y R030 del registro) y **no pasó**. Lo que muestra el video no cambia ese resultado: las diferencias que se ven son chicas o van en contra de sus propias reglas.

## 2. Qué hay en la pantalla

### 2.1 Herramientas visibles

| Herramienta | Qué se ve | Marca |
|---|---|---|
| **Plataforma de operación** | No es TradingView. Interfaz en italiano («Qtà aperta», «P/L giornaliero», «Annulla e chiudi posizioni»), aviso «Attiva Windows», panel de órdenes con MKT, BID/ASK, LMT, STP, STP LMT, «Breakeven» y «OCO Strategy». No se ve el nombre. En el navegador hay pestañas «Platform» con el ícono de IQ, así que **probablemente sea la plataforma de IQCapital**, la empresa que hace el video. | [muestra] / [inferido] |
| **Instrumentos** | Pestañas «MNQ 5 m» y «MES 5 m» (después «MES 15 m»). Al principio, MNQ en 15 minutos («15 Minuti»). | [muestra] |
| **Perfil de volumen por sesión** | Un histograma por día pegado al costado izquierdo de cada sesión. Naranja adentro del área de valor y amarillo afuera. Línea **magenta** = POC. Líneas **celestes punteadas** = bordes del área de valor (VAH arriba, VAL abajo). Una línea **amarilla punteada** dentro del área, que no explica. | [muestra]; qué es cada color: [inferido] |
| **Perfil por vela** | Cerca del final cambia el gráfico a velas con un mini perfil de volumen adentro de cada una. No lo usa para decidir. | [muestra] |
| **Regla de medición** | Muestra «velas, puntos (ticks), %». Ejemplos: «15,50 (62 ticks)», «10,50 (42 ticks)», «9,75 (39 ticks)», «75,00 (300 ticks)», «63,75 (255 ticks)». | [muestra] |
| **Dibujo a mano** | Líneas horizontales con precio (31412,50; 31304,50; 31284,00), rectángulos amarillos sobre las zonas de consolidación y una nota de texto con la cuenta de 1/√N. | [muestra] |
| **TradingView** | Solo aparece como una **pestaña del navegador** con «ES1!» (así nombra TradingView al futuro continuo del S&P) y como marcador. **No se lo ve usar** para analizar ni operar. | [muestra] |
| **Caliber** (caliberterminal.com) | Web de análisis estadístico de un historial de operaciones, abierta con la cuenta «gianlucabruni.g». Estrategias guardadas: «AXIOM NQ», «Axiom», «Championship», «PK» y «test». No sabemos si es un producto suyo o de terceros. | [muestra]; de quién es: no verificado |

**Para el dueño, si piensa pagar TradingView:** en este video **no hay nada** que dependa de un plan pago de TradingView, porque no lo usa. Lo que él usa (perfil de volumen por sesión, área de valor, POC y una regla que mide en ticks) tiene equivalentes en TradingView, pero en qué plan está cada uno hay que mirarlo en la página oficial de planes. Acá no lo suponemos.

### 2.2 Fechas y horas

- El gráfico muestra la sesión del **07-10-2026** [muestra]. El video se publicó el 30-10, así que se grabó unas tres semanas antes [inferido].
- La primera entrada se ve a las **16:30** del eje del gráfico. Imre dice que van «casi una hora de sesión», y la sesión de Nueva York abre a las 15:30 en Italia en esa fecha. Así que el eje está en **hora de Italia (UTC+2)**: la entrada fue alrededor de las **14:30 UTC** [inferido].

## 3. Línea de tiempo con lo que se ve y lo que dice

Los links abren el video en ese segundo.

| Momento | Qué se ve | Qué dice | Lectura |
|---|---|---|---|
| [0:30](https://youtu.be/2WZ137MtCiw?t=30) | — | Va a buscar cortos en el Nasdaq. Como hay pocas horas, no usa su estrategia principal sino otra de momentum, «para ganarle a la varianza» con muchas operaciones. | [dice] Admite que cambia de estrategia por el formato del video. |
| [1:30](https://youtu.be/2WZ137MtCiw?t=90)–[3:30](https://youtu.be/2WZ137MtCiw?t=210) | MNQ en 15 min, perfiles de tres sesiones (05, 06 y 07-10). Dibuja una línea en **31412,50**. | El mercado venía alcista y rompió ese nivel para abajo. El área de valor de hoy «se está formando debajo de ese nivel». El POC de antes era «el de los largos». | [muestra + dice] Usa el nivel roto y la posición del área de valor de hoy como **contexto de dirección**, no como disparador. |
| [9:30](https://youtu.be/2WZ137MtCiw?t=570) | — | «Hoy solo usamos el perfil de volumen» para tener clara la dirección. | [dice] |
| [10:00](https://youtu.be/2WZ137MtCiw?t=600)–[11:30](https://youtu.be/2WZ137MtCiw?t=690) | Cambia a **5 minutos** (MES y MNQ). Mide con la regla una vela chica del ES: «2,25 (9 ticks)». En el MNQ quedan dos líneas: **31412,50** y **31304,50**. En el ES, 7867,75 y 7853,00. | Su sistema es de 15 minutos; baja a 5 «para tener más setups». Primera condición: una sesión muy direccional. Las dos líneas son la estructura **actual** y la **periférica**, y el precio rompió las dos. | [muestra + dice] La más baja (31304,50) sería la actual y la de arriba (31412,50) la periférica [inferido]. |
| [11:30](https://youtu.be/2WZ137MtCiw?t=690)–[12:30](https://youtu.be/2WZ137MtCiw?t=750) | — | Necesita «10 minutos de momentum»: **dos velas de 5 min** seguidas. El momentum se mide «por cuántos ticks cae la vela». Eligió el umbral como uno que «cuadra con el backtest», no el mejor, para no sobreajustar. | [dice] |
| [18:00](https://youtu.be/2WZ137MtCiw?t=1080) | Gráfico de 5 min con un rectángulo amarillo sobre una zona (31262–31273,75). | Ya hay una primera vela bajista; falta la segunda. | [muestra + dice] |
| [19:00](https://youtu.be/2WZ137MtCiw?t=1140)–[19:45](https://youtu.be/2WZ137MtCiw?t=1185) | **Corto de 10 MNQ en 31239,50.** Stop en **31286,00** («−930 $»). Objetivo en **31164,50** («+1.500 $»). La regla mide el objetivo: «75,00 (300 ticks)». | Stop «arriba de las dos velas», en el máximo de la primera. Objetivo fijo «1 a 1,5». Quería arriesgar 1.000 USD y calculó los contratos «de cabeza». | [muestra] Stop de 46,5 puntos y objetivo de 75: **1,61 R**, no 1,5. Con 10 MNQ (2 USD por punto cada uno): 930 y 1.500 USD, coinciden con la pantalla. |
| [20:40](https://youtu.be/2WZ137MtCiw?t=1240) | La regla sobre la segunda vela: «**15,50 (62 ticks)**». | «Necesitamos dos velas de al menos 60 ticks.» | [muestra] Confirma que los 60 ticks son **15 puntos** (un tick del NQ = 0,25 puntos). |
| [21:00](https://youtu.be/2WZ137MtCiw?t=1260) | — | En 15 minutos el setup todavía no está: «nuestra operación es un poco temprana». | [dice] Entró sin la señal de su marco de tiempo probado. |
| [23:30](https://youtu.be/2WZ137MtCiw?t=1410) | — | Si sale la misma señal en 15 minutos, toma otra operación y arriesga más. Si salen otras dos velas bajistas, abre más posición. | [dice] |
| [25:00](https://youtu.be/2WZ137MtCiw?t=1500)–[27:30](https://youtu.be/2WZ137MtCiw?t=1650) | Escribe una nota en el gráfico: «10 setup uncertainty so high · 1/10 31% · 1/1000 31 0.03 60 · 57 · 63». | Con 60 % de aciertos a 1:1, en 10 operaciones la incertidumbre es 1/√10 ≈ 31 %; en 1.000, 1/√1000 ≈ 3 %, o sea entre 57 % y 63 %. | [muestra + dice] Ver §5.1. |
| [33:00](https://youtu.be/2WZ137MtCiw?t=1980)–[33:40](https://youtu.be/2WZ137MtCiw?t=2020) | Mide dos velas bajistas seguidas: «**10,50 (42 ticks)**» y «**9,75 (39 ticks)**». | No son señal: «necesitamos 60 ticks». «No miro si un setup es lindo; miro si es válido o no.» | [muestra + dice] Buen ejemplo para la Academia: dos velas a favor que **no** alcanzan. |
| [37:00](https://youtu.be/2WZ137MtCiw?t=2220) | El panel muestra pérdida del día de **940 USD**. | «Creo que tocó el stop.» | [muestra] 930 del stop más comisiones. |
| [37:30](https://youtu.be/2WZ137MtCiw?t=2250)–[38:40](https://youtu.be/2WZ137MtCiw?t=2320) | Un rectángulo grande arriba (aprox. 31360–31450) y una línea en **31284,00**. El precio cierra arriba de 31284. **Largo de 10 MNQ en 31294,50**, stop en **31247,50** («−940 $»), objetivo en **31372,50** («+1.560 $»). | La zona de abajo se rompió hacia arriba: hay una vela verde válida y esta es la segunda. Va «hasta este nivel» (el rectángulo de arriba), «contra el área de valor». «No es el mejor setup, pero tenemos que operar.» | [muestra + dice] Stop de 47 puntos y objetivo de 78: **1,66 R**. El borde de abajo del rectángulo coincide con el VAH de la sesión (≈31360) [inferido]. |
| [40:30](https://youtu.be/2WZ137MtCiw?t=2430)–[42:30](https://youtu.be/2WZ137MtCiw?t=2550) | Dibuja rectángulos sobre dos consolidaciones y va señalando vela por vela. | Explica la zona: buscar **3 velas válidas del color contrario** (verdes, si busca cortos). Vela válida: el cuerpo es más grande que la mecha más larga. La zona más reciente es la actual y la siguiente hacia atrás con 3 velas es la periférica. | [muestra + dice] Es la definición de estructura del informe (§3.B). |
| [42:30](https://youtu.be/2WZ137MtCiw?t=2550)–[43:20](https://youtu.be/2WZ137MtCiw?t=2600) | — | Ahora la actual es alcista y la periférica bajista. Prefiere esperar que coincidan, pero puede operar la actual «hasta el nivel del 75 % o el VAH». | [dice] Esto explica el largo: es la excepción que él mismo dice preferir no tomar. |
| [43:30](https://youtu.be/2WZ137MtCiw?t=2610)–[46:30](https://youtu.be/2WZ137MtCiw?t=2790) | **Caliber**, estrategia «AXIOM NQ» (detalle en §4). | Es para prop firms: importa operar seguido, no la expectativa alta. Probablemente **no le gana** a comprar y mantener. | [muestra + dice] |
| [46:40](https://youtu.be/2WZ137MtCiw?t=2800)–[47:30](https://youtu.be/2WZ137MtCiw?t=2850) | Mide una vela: «6,50 (26 ticks)», no alcanza. | Si la próxima vela tiene 60 ticks o más, suma tamaño y sube el stop de toda la posición. | [muestra + dice] |
| [47:50](https://youtu.be/2WZ137MtCiw?t=2870)–[48:10](https://youtu.be/2WZ137MtCiw?t=2890) | Después de una vela verde grande: **22 MNQ** con precio medio **31313,32**. Stop subido a **31269,75** («−1.914 $»). El objetivo queda en 31372,50 («+2.607 $»). La regla mide la distancia al stop: «63,75 (255 ticks)». | Dice que sumó «ocho» contratos. El stop ahora está cerca de 2.000 USD, así que el objetivo pasa a ser 3.000 USD. | [muestra] Pasó de 10 a **22** contratos (sumó 12, no 8), a unos 31329 [inferido por el precio medio]. En ese momento la posición quedó en **1,36 R**. |
| [49:00](https://youtu.be/2WZ137MtCiw?t=2940) | — | Llegamos al VAH. Si acá sale un disparador corto, cierra el largo y se da vuelta. «El largo era solo hasta este nivel.» | [dice] Regla de salida extra que no está en el backtest. |
| [52:30](https://youtu.be/2WZ137MtCiw?t=3150) | Velas con perfil de volumen por vela. | «Esta vela no cierra arriba de 60 ticks», así que no suma más. | [muestra + dice] |
| [53:20](https://youtu.be/2WZ137MtCiw?t=3200)–[53:50](https://youtu.be/2WZ137MtCiw?t=3230) | El objetivo aparece **movido a 31381,50**. Se cierra la posición: P/L del día **+2.038,00 USD**. | «Hicimos 3.000, pero tuvimos un stop.» El presentador dice «2.100». | [muestra] 22 × (31381,50 − 31313,32) × 2 USD ≈ 3.000 USD. **Movió el objetivo** para que diera la cifra redonda. |

## 4. El backtest que muestra (Caliber, «AXIOM NQ»)

### 4.1 Lo que se lee [muestra]

| Dato | Valor |
|---|---|
| Período | **07-01-2026 → 30-09-2026** («Whole record») |
| Operaciones | 405 |
| Resultado | +50,0 R; +59,8 % arriesgando 1 % por operación |
| Expectativa | +0,12 R por operación |
| Aciertos | 44,9 % (40 % para quedar en cero) |
| Ganadora y perdedora media | +1,50 R y −1,00 R (sin operaciones en cero) |
| Profit factor | 1,22 |
| t-estadístico | 2,00, «significativo al 95 %» |
| Caída máxima | −21,4 % de la cuenta; −23,5 R en el peor tramo |
| Tiempo bajo el máximo anterior | 87,4 % de las operaciones |
| Racha perdedora máxima | 10 |
| Sharpe y Sortino anualizados | 2,20 y 3,69 |
| CAGR anualizado | +90,3 % |
| Concentración | 15 % de la ganancia viene de las 5 mejores operaciones |
| Últimas 100 operaciones | +0,25 R por operación |
| «Suerte en su mejor caso» | +40 R (1 de cada 20 caminos sin ventaja termina ahí) |
| «Risk headroom» | «1,00 % → 0,55 %»: la propia web dice que **1 % está por encima de lo que este historial justifica** |
| Probabilidad de ir perdiendo | 16 % a las 100 operaciones; 5,9 % a las 249; menos de 1 % recién a las **551** |

### 4.2 ¿Cierran los números?

Sí. Con 44,9 % de aciertos, +1,5 R y −1 R:

- expectativa = 0,449 × 1,5 − 0,551 = **+0,12 R**, y 405 × 0,123 ≈ **50 R**;
- desvío por operación = 2,5 × √(0,449 × 0,551) ≈ **1,24 R**, error estándar = 1,24 / √405 ≈ 0,062, t ≈ 0,123 / 0,062 ≈ **2,0**;
- P(ir perdiendo tras 100) = Φ(−0,123 × 10 / 1,24) ≈ **16 %**; tras 249 ≈ **5,9 %**; para bajar de 1 % hacen falta (2,33 × 1,24 / 0,123)² ≈ **550** operaciones;
- «suerte en su mejor caso» = 1,645 × 1,24 × √405 ≈ **41 R**.

### 4.3 Por qué no alcanza para creerle

1. **Es un solo período de nueve meses.** No hay walk-forward ni un tramo fuera de muestra a la vista. Nuestras reglas (CLAUDE.md §5b) piden varios períodos y un set final intocable.
2. **El «test de suerte» es el mismo t-test dicho de otra forma.** Superar los +40 R de «suerte» equivale a t ≈ 1,65, y él tiene t = 2,0. No es una prueba extra.
3. **Varias estrategias en la misma cuenta** («Axiom», «Championship», «PK», «test»). Si se probaron muchas variantes, un t de 2,0 en una sola es lo que se espera por azar. Faltaría el Sharpe deflactado con la cantidad real de pruebas.
4. **«Probabilidad de ir perdiendo»** supone que la ventaja medida es la verdadera. Si es más chica, esas probabilidades son mucho peores.
5. **Los costos no se ven.** No sabemos si los +0,12 R son antes o después de comisión y slippage. En el video, el panel muestra unos 10 USD de comisiones por ida y vuelta con 10 MNQ.
6. **No sabemos si son operaciones reales o simuladas.** La web habla de «track record» y tiene un botón «Log a trade», pero él lo llama backtest.
7. **El Sharpe de 2,20 es el mismo número** que dio para su estrategia «de campeonato» en la entrevista del 02-10-2026 (informe, §3.C). Puede ser casualidad, pero conviene no sumar las dos cifras como si fueran pruebas independientes.

## 5. Interpretación: la estrategia A, armada con lo que se ve

### 5.1 La idea

1. **Dirección del día.** Mira las dos últimas zonas de consolidación rotas: la actual y la periférica. Si las dos se rompieron para el mismo lado, ese es el lado del día. El perfil de volumen confirma: el área de valor de hoy se forma del lado de la ruptura [dice + muestra].
2. **Disparador.** Dos velas seguidas a favor, cada una con un **cuerpo de al menos 60 ticks** (15 puntos del NQ), medidas con la regla. Lo probó en **15 minutos**; en el video lo usó en 5 [dice + muestra].
3. **Entrada** al cierre de la segunda vela [muestra: la entrada coincide con ese cierre].
4. **Stop** en el extremo de la **primera** vela del impulso [dice; muestra: el stop del corto queda en el máximo de esa vela, con una diferencia de un punto que no se puede resolver con el video].
5. **Objetivo fijo** de 1,5 R [dice]. En pantalla queda entre 1,6 y 1,7 R porque lo fija en dólares [muestra + inferido].
6. **Tamaño:** contratos para perder un monto fijo (1.000 USD «de cabeza») [dice]. En la pantalla, 930 y 940 USD [muestra].
7. **Piramidación:** si aparece otra vela válida de 60 ticks a favor, suma contratos y mueve el stop de toda la posición [dice + muestra]. No dice si hace falta una vela o dos (en un momento dice «si esta vela tiene 60 ticks» y en otro «nunca tuvimos dos velas seguidas»).
8. **Salida extra:** si el precio llega al VAH o a la zona periférica y sale un disparador en contra, cierra y se da vuelta [dice]. No se vio pasar.

### 5.2 Lo que corrige el video a nuestras notas anteriores

| Antes decíamos | Lo que muestra el video |
|---|---|
| «Opera momentum en el futuro del Nasdaq (NQ)» | Opera el **micro (MNQ)**: 2 USD por punto, la décima parte del NQ. Las reglas en ticks son las mismas. |
| «Una operación perdida, una ganada con piramidación y una más pequeña» | **Dos operaciones**: el corto perdido y el largo, que tuvo **una suma** de contratos. |
| «Termina la sesión con unos +2.100 USD» | El panel dice **+2.038 USD** al cierre del día. |
| «Arriesga 1.000 USD» | Lo dice, pero la pantalla muestra 930 y 940 USD por operación y, después de sumar, **1.914 USD**. |
| «Objetivo fijo de 1,5 R» | En las órdenes queda en **1,61 R** y **1,66 R**, y al piramidar baja a **1,36 R** hasta que mueve el objetivo. |
| «Usa la regla de TradingView» (dicho para la entrevista del 02-10) | En este video **no usa TradingView**. La regla de su plataforma mide puntos y ticks juntos. |
| Período del backtest sin dato | **Nueve meses** (enero–septiembre de 2026). |

**Sobre el «6 para el Nasdaq» de la estrategia C** (informe, §3.C): ese margen **no aparece en este video**, así que no se puede confirmar desde acá. Lo que sí se ve es que **siempre mide en ticks con la regla** y que su plataforma muestra los ticks entre paréntesis. Eso refuerza, sin probarla, la lectura de **6 ticks (1,5 puntos)**. Sigue a confirmar.

## 6. Alertas

1. **Es contenido de una empresa de fondeo** que vende desafíos de 9 USD, con publicidad en el medio y al final. La plataforma que usa probablemente sea de esa misma empresa [inferido].
2. **No siguió su sistema:** bajó de 15 a 5 minutos, entró antes de que hubiera señal en 15 minutos, tomó un largo con la estructura desalineada y movió el objetivo después de sumar contratos.
3. **Duplicó el riesgo en una operación** al piramidar, de 940 a 1.914 USD. En nuestro proyecto eso superaría el 1 % por operación si la primera ya estaba en 1 %.
4. **Los 20.000 USD de Imre** son de una posición abierta una semana antes y sin cerrar. El mismo video lo aclara.
5. **Una sesión es un dato.** Lo dice Bruni y lo dice Imre. Ganar ese día no prueba nada.

## 7. Para la Academia

Temario sugerido, de lo más simple a lo más difícil. Cada punto se puede enseñar con un fragmento del video (minuto entre paréntesis) y practicar con el modo repetición de la terminal.

1. **Ticks y puntos** (20:40, 33:00). En el NQ un tick son 0,25 puntos. 60 ticks son 15 puntos. Ejercicio: medir velas y decir cuántos ticks tienen.
2. **Vela válida** (41:30). El cuerpo es más grande que la mecha más larga. Ejercicio: marcar las velas válidas de un tramo.
3. **Zona y ruptura** (40:30–42:00). Una zona necesita 3 velas válidas del color contrario. La ruptura cuenta solo si el **cuerpo** cierra afuera. Ejercicio: dibujar el rectángulo de la zona.
4. **Estructura actual y periférica** (11:00, 42:00). La última zona rota y la anterior. Cuándo están alineadas y cuándo no.
5. **Perfil de volumen por sesión** (1:30–3:30). Área de valor (70 % del volumen), POC, VAH y VAL. Qué quiere decir que el área de valor de hoy se forme debajo de la de ayer.
6. **El disparador de dos velas** (19:00, 33:00). Un caso que entra (62 ticks) y uno que no (42 y 39 ticks). «Válido o no válido», nunca «lindo o feo».
7. **Stop, objetivo y tamaño** (19:45). Stop en la primera vela, objetivo de 1,5 R, contratos según el monto que se arriesga. Ejercicio: calcular los contratos para perder 1 % con un stop de 46,5 puntos.
8. **Incertidumbre 1/√N** (25:00–27:30). Con 10 operaciones, cualquier resultado es posible. Hacen falta cientos.
9. **Leer un backtest con ojo crítico** (43:30–46:30). Qué dice un t de 2,0, por qué nueve meses no alcanzan y por qué el «test de suerte» no es una prueba extra.
10. **Piramidación y riesgo** (47:30–53:50). Cómo sumar contratos cambia el riesgo y la relación riesgo/beneficio. Por qué el proyecto no la usa sin pasar por el módulo de riesgo.
11. **Disciplina contra formato** (todo el video). Qué cambió él por estar en cámara, y por qué eso no es lo que dice su backtest.

## 8. Para backtestear

Definiciones objetivas según CLAUDE.md §5b. Las velas de 15 minutos son las que él dice haber probado. **Aviso:** H4.1 y H4.2 ya probaron esta estrategia y no pasaron (R029, R030). El pre-registro del programa 4 **prohíbe usar la reserva en variantes** de esas hipótesis. Cualquier prueba nueva necesita un pre-registro nuevo, con fecha y con su parte del presupuesto global, y que pase antes el filtro de borde bruto (`E_bruto ≥ 2 × C`). Lo decide el dueño.

| # | Regla | Definición medible | Lo que tomó H4.1 |
|---|---|---|---|
| A1 | Vela válida | `|cierre − apertura| > max(mecha superior, mecha inferior)` | No se exigía (punto abierto 7) |
| A2 | Zona | Tramo de velas cerradas con al menos 3 velas válidas del color contrario a la operación. Límites: máximo y mínimo de ese tramo. | Igual (§3 del pre-registro) |
| A3 | Ruptura | El cuerpo de una vela cerrada queda entero afuera de la zona (el cierre pasa el límite). | Igual |
| A4 | Estructura actual y periférica | Actual = la última zona rota; periférica = la anterior. Alineadas si las dos rupturas van para el mismo lado. | Igual |
| A5 | Disparador | Velas `t−1` y `t` cerradas, del color de la operación, con cuerpo ≥ 60 ticks (15 puntos) cada una; opcional: las dos válidas (A1). | Igual sin A1 |
| A6 | Entrada | Al precio de apertura de `t+1` (en vivo, él entra al cierre de `t`, que es casi lo mismo). | Igual |
| A7 | Stop | Máximo de `t−1` en cortos, mínimo de `t−1` en largos. | Igual |
| A8 | Objetivo | Entrada ± 1,5 × R. | Igual (no se copia el objetivo en dólares) |
| A9 | Piramidación | Con la posición en ganancia, otra vela a favor con cuerpo ≥ 60 ticks: se suma tamaño y el stop de todo pasa al extremo de esa vela. **Riesgo total ≤ 1 % de la cuenta.** | No se tomó |
| A10 | Salida en el VAH | Si el precio toca el VAH de la sesión (en largos) y aparece un disparador A5 en contra, se cierra. | No se tomó |
| A11 | Operar con estructura desalineada | Se permite solo hasta el VAH o el 75 % del tramo, con objetivo en el menor de los dos. | No se tomó |

**Preguntas abiertas (hay que fijarlas antes de mirar datos):**

1. **Piramidación:** ¿una vela válida o dos seguidas? ¿Cuántas sumas como máximo? ¿Cuántos contratos en cada suma? En el video pasó de 10 a 22. Y el objetivo: ¿queda donde estaba o se recalcula?
2. **Perfil de volumen:** en el gráfico cada perfil arranca en el cambio de día, a medianoche de Italia [muestra]. En octubre eso son las 18:00 de Nueva York, o sea **la sesión del CME** [inferido]. Hay que confirmar que no sea solo el horario de Nueva York.
3. **Línea amarilla punteada** del perfil: ¿qué es? No lo explica.
4. **«Nivel del 75 %»:** ¿75 % de qué tramo exactamente?
5. **Mínimo de distancia de la ruptura:** lo menciona en otras entrevistas sin dar número.
6. **Costos:** su backtest no muestra si los incluye. Para el US100 de IG hay que usar spread, slippage y swap reales (`config/`).
7. **El umbral de 60 ticks** se eligió con datos recientes (NQ cerca de 25.000). Para años anteriores ya se probó la versión normalizada por ATR (H4.2, k = 0,35), y no pasó.

## 9. Fuentes

- Video: https://www.youtube.com/watch?v=2WZ137MtCiw (IQCapital, 30-10-2026), fotogramas y subtítulos automáticos en inglés, revisados el 26-11-2026.
- Informe general: `docs/research/2026-11-gianluca-bruni.md`.
- Apuntes anteriores del mismo video: `docs/research/videos/2026-11-25-emre-vs-gianluca.md`.
- Pre-registro y resultados del programa 4: `docs/analysis/2026-11-programa-4-preregistro.md`, `docs/analysis/2026-11-programa-4-h41-h42-resultados.md`, filas R029 y R030 de `docs/analysis/registro-de-resultados.csv`.
