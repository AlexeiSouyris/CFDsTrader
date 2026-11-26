# Academia: curso interactivo, herramientas que se desbloquean y exámenes con el pasado (propuesta)

> **Estado: propuesta (#709, eje 2 de la épica #707).** Solo documentación: no hay código, config
> ni contratos nuevos en este PR. Todo número marcado **PENDIENTE** es un parámetro del dueño y no
> se usa hasta que lo apruebe en el Issue o en el PR. Los contratos se definen después, en su propio
> PR (contratos primero, CLAUDE.md §1).
>
> **Nada de esto es consejo de inversión** ni promete ganancias. La Academia enseña a medir, no a
> ganar. Todo pasa en simulación (CLAUDE.md §5).

## 1. Qué pidió el dueño y qué se propone

Pedido (26-11-2026, resumen): un curso completo que empiece por lo más simple y crezca a lo más
complejo, muy interactivo; que enseñe a armar soportes y a usar indicadores y herramientas de una
terminal; que las herramientas **se desbloqueen** a medida que aprende; **exámenes prácticos con el
pasado** y ejemplos ultra realistas; cómo armar una estrategia propia y operar **cuentas
fondeadas**; cómo **ganarle a la varianza**; y aprender de quienes **sí tienen pruebas**.

La propuesta, en cinco piezas:

1. **Temario en 13 niveles (0 a 12)**, de "qué es un mercado" a "cuentas fondeadas" (§3).
2. **Desbloqueo**: cada herramienta de la Terminal queda bloqueada hasta aprobar su lección; el
   dueño puede saltarlo en Ajustes (§4).
3. **Exámenes con replay**: velas reales guardadas, fecha oculta al azar, vela por vela; el
   estudiante marca niveles, entradas y stops, y **el motor en Rust califica** sin mirar el futuro
   (§5).
4. **Simulador de cuentas fondeadas**: reglas configurables, las operaciones del estudiante o de una
   estrategia pasan por esas reglas, y Monte Carlo da la probabilidad de pasar (§6).
5. **Fuentes con evidencia** por nivel, sin influencers; nuestro propio registro de resultados como
   caso de estudio (§7).

### Principios (no negociables)

- **La UI no calcula lógica de trading** (CLAUDE.md §1). Puntajes de exámenes, R, costos, niveles
  "correctos" y simulaciones de prop firms los calcula **el motor**. La UI solo muestra.
- **Proceso antes que resultado.** Se califica si se siguió la regla (stop, tamaño, costos, plan),
  no si la operación ganó. Una operación ganadora sin stop **reprueba**; una perdedora bien hecha
  **aprueba**. Es la misma idea que los trofeos de proceso (`docs/design/2026-11-trofeos-reglas.md`).
- **Sin mirar el futuro** en ningún ejercicio: el replay nunca manda una vela que el estudiante no
  debería ver todavía (CLAUDE.md §5b).
- **Definiciones objetivas**: cada concepto se enseña con la definición medible que ya usa el motor
  (pivote confirmado con N velas, FVG, order block...), no con "se ve como".
- **Honestidad con la evidencia**: cuando algo no funcionó en nuestras pruebas, la lección lo dice
  con el número del registro (`docs/analysis/registro-de-resultados.csv`).
- **El fuera de muestra no se toca**: los exámenes no usan los últimos 12 meses reservados en
  `config/data_split.toml` (§5.3).

## 2. Cómo es una lección

Cada lección tiene siempre las mismas cinco partes:

| Parte | Qué es | Quién calcula |
|---|---|---|
| **Objetivo** | Una frase: qué vas a saber hacer al terminar. | — |
| **Interactivo** | Algo para tocar: un gráfico real, un deslizador, un replay corto, la calculadora del ticket. Reusa componentes que ya existen (`LiquidityChart`, menú Herramientas, ticket, Multigráfico). | El motor manda los datos; la UI dibuja |
| **Ejercicio** | Práctica sin nota, con la solución del motor al lado. | Motor |
| **Examen** | Con nota y umbral de aprobación (**PENDIENTE**, propuesta: 80 % en preguntas, "proceso aprobado" en replays). Se puede repetir; cada intento queda guardado. | Motor |
| **Desbloquea** | La herramienta de la Terminal que se habilita al aprobar. | Motor (estado de progreso) |

Además, **un hábito por nivel** (transversal, desde el nivel 3): escribir el plan antes de cada
ejercicio de replay y una nota en el Diario después. El curso no separa la psicología al final:
se practica desde el principio.

El texto de cada lección vive en Markdown en el repo (`docs/academia/nivel-NN-*.md`, propuesto), igual
que el glosario: la UI lo lee y los términos enlazan a `docs/GLOSARIO.md` (única fuente de las
definiciones). Las claves de las respuestas **no** viajan a la UI: las tiene el motor.

## 3. Temario

Los números de costos y riesgo salen de nuestra config real (`config/instruments.toml`,
`config/risk.toml`, `config/research.toml`), con la fuente que ya tiene cada valor. Si la config
cambia, la lección muestra el valor nuevo: la UI lo pide al motor, no lo copia.

### Nivel 0 — Orientación: qué es esto y por qué la mayoría pierde

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 0.1 Qué es un mercado | Entender que del otro lado hay alguien, y que el precio es el último acuerdo. | Lista de mercados (watchlist) en solo lectura con el último cierre del motor. | Leer último, variación y hora de 3 instrumentos. | 5 preguntas. | **Watchlist** y panel del instrumento |
| 0.2 Cuántos pierden | Saber el dato duro antes de empezar: ESMA (2018) midió que entre 74 % y 89 % de las cuentas minoristas de CFD pierden dinero. | Tarjeta con la cita y su fuente. | — | 3 preguntas. | **Glosario** interactivo |
| 0.3 Cómo se usa la Academia | Simulación siempre, proceso antes que resultado, nada de promesas. | Recorrido por la Terminal con todo bloqueado y el porqué de cada candado. | — | — | — |

### Nivel 1 — Instrumentos: contado, futuros y CFD; apalancamiento y margen

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 1.1 Contado, futuros y CFD | Saber qué se compra en cada uno: el activo, un contrato con vencimiento o una diferencia de precio con un broker. | Mismo día en el CFD US500 (IG) y en el futuro ES (Databento) lado a lado: precio, vencimiento, roll (`mark_rollovers`). | Encontrar el salto del vencimiento en el futuro y explicar por qué el CFD no lo tiene. | 6 preguntas. | **Selector de fuente** (CFD / futuros) en los gráficos |
| 1.2 Largo y corto | Ganar si sube o si baja, y qué se pierde en cada caso. | Deslizador de precio sobre una posición simulada; el P/L lo calcula el motor. | 4 casos. | 5 preguntas. | — |
| 1.3 Apalancamiento y margen | Entender margen minorista 5 % (= 20:1, ESMA) y nuestro **tope propio de 10:1** (spec §8), y el cierre por margen al 50 % (`margin_closeout_pct`). | Calculadora: tamaño → margen y apalancamiento efectivo (del `ticket_verdict.v1` del motor). | Encontrar el tamaño máximo que permite el tope de 10:1 en US100. | 6 preguntas + 2 cálculos. | — |

### Nivel 2 — Costos reales: lo que se come la ventaja

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 2.1 Spread | Saber que se entra pagando. Nuestros spreads típicos: US500 0,9 pts, US100 1,6, US30 3,7, DE40 1,8; oro 0,9 mínimo (`config/instruments.toml`, fuentes IG y medición en la demo, #67). | El spread dibujado sobre una vela real de 15 m: cuánto de la vela "es costo". | Expresar el spread en R para un stop de 1 ATR. | 4 cálculos. | **Columna de costos** del ticket |
| 2.2 Financiación nocturna (swap) | Calcular el costo de dormir con una posición: tipo de referencia (SOFR en EE. UU., €STR en DE40) + 3 % de IG, base 360 en índices; el oro usa la base de la curva de futuros (ADR 0013). | Deslizador de noches; el motor calcula con la serie real de `reference_rates` del día elegido. | Ejemplo con números redondos: precio 6.000, tasa 4 % + 3 % = 7 % → 6.000 × 7 % / 360 ≈ 1,17 pts por noche y por unidad. | 3 cálculos. | — |
| 2.3 Slippage y comisión | Saber que el precio de la orden no es el precio que te dan; acciones con comisión (0,02 USD por acción, mínimo 15). | Replay de una noticia fuerte con la ejecución en la vela siguiente. | — | 3 preguntas. | — |
| 2.4 Caso real: los costos mataron la estrategia | Leer un resultado propio: la estrategia en uso da casi 0 R antes de costos y **−0,12 a −0,19 R por operación** después (R001–R003). El momentum de la última media hora: t −11 que viene **solo del costo** (R017, R026). La deriva nocturna existe pero el costo del CFD es 2 a 4 veces mayor (R021). | Tabla del registro con "antes de costos / costos / neto". | Ordenar 3 estrategias por cuánto les come el costo. | 5 preguntas. | **Calculadora de costos** (preview del ticket con costos) |

### Nivel 3 — Velas, timeframes y el primer replay

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 3.1 La vela (OHLC) | Leer apertura, máximo, mínimo y cierre; vela cerrada frente a vela en formación. | Vela en formación de IBKR (`ibkr_live_bar.v1`) frente a la cerrada. | Identificar 5 velas por su OHLC. | 5 preguntas. | **Gráfico de velas** y OHLC bajo el cursor |
| 3.2 Timeframes | La misma hora en 1 m, 15 m, 1 h y 1 d; la vela mayor solo vale cuando cerró. | Multigráfico 2 × 2 con el mismo momento. | Decir qué vela de 1 h existe a las 10:40 UTC. | 4 preguntas. | **Selector de timeframe** y **Multigráfico** |
| 3.3 Primer replay | Avanzar vela por vela sin ver el futuro; escribir el plan antes. | Replay sin nota, 200 velas, fecha oculta (§5). | Escribir qué esperás antes de avanzar 20 velas. | — (práctica) | **Replay libre**, **regla de medición** |
| Hábito | Plan escrito antes, nota en el Diario después. | — | — | — | **Notas del Diario** |

### Nivel 4 — Estructura: pivotes, soportes, resistencias y tendencia

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 4.1 Pivote confirmado | Un máximo o mínimo local existe solo después de N velas de confirmación (`config/patterns.toml`); antes es una suposición. | Replay donde el pivote "aparece" recién en la vela N. | Marcar dónde se confirma cada pivote. | Replay: marcar 5 pivotes; el motor compara con los suyos. | **Capa de pivotes** del motor |
| 4.2 Soporte y resistencia objetivos | Armar un nivel como lo define el motor: tocado ≥ k veces con tolerancia X % (`levels` de `patterns`). | Dibujás tu línea; después se prende la capa del motor y se ve la diferencia. | Armar 3 niveles en un día real. | **Replay de niveles** (§5.4): precisión y cobertura frente a los niveles del motor, con tolerancia en ATR. | **Líneas horizontales, de tendencia y rectángulos** (dibujo) |
| 4.3 Tendencia y ruptura | Máximos y mínimos crecientes; ruptura y retesteo; doble techo y suelo. | Capa de tendencia de 1 h del motor. | Clasificar 10 tramos. | 6 preguntas + 1 replay. | **Capa de soportes y resistencias** y patrones chartistas |
| Evidencia | Los soportes en divisas tienen algo de poder (Osler, 2000) y los números redondos concentran órdenes (Osler, 2003). Nuestro M1 "cruce de número redondo" **no pasó** (R020). | — | — | — | — |

### Nivel 5 — Liquidez: niveles, barridas, FVG y order blocks (con advertencias)

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 5.1 Mapa de liquidez | PDH/PDL, PWH/PWL, EQH/EQL, máximos y mínimos de sesión, números redondos, y su estado intacto/barrido/roto (`liquidity_level.v1`). | Capa del mapa en un día real. | Predecir cuál se barre primero (sin nota). | 6 preguntas. | **Mapa de liquidez** |
| 5.2 Sweep, BOS y CHoCH | Definición medible de barrida y de ruptura de estructura del motor (`liquidity`, spec §5.4). | Replay con los eventos marcados al confirmarse. | Marcar barridas. | Replay: marcar 5 eventos; el motor compara. | **Capas de sweep y estructura** |
| 5.3 FVG y order blocks | Qué son con la regla del motor; estadísticas de toque, mitad y llenado con IC de Wilson (`liquidity_stats.v1`). | Tabla de estadísticas por instrumento y timeframe. | Encontrar 3 FVG y decir cuándo se llenaron. | 5 preguntas. | **Capas de FVG y order blocks**, **estadísticas de FVG** |
| 5.4 Caso real: el FVG frente al placebo | La reacción al primer toque de un FVG de 15 m **no supera a un placebo** emparejado (R035: −0,023 ATR, p 0,96); en 1 h, menos que el placebo (R036); filtrar la estrategia con FVG no mejora (R037, R038). | Gráfico real − placebo. | Explicar qué es un placebo y por qué hace falta. | 4 preguntas. | — |

### Nivel 6 — Indicadores

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 6.1 ATR | Medir el tamaño "normal" del movimiento (Wilder, 1978); base de stops y tamaños. | ATR(14) del motor sobre el gráfico; deslizador de período (el motor recalcula). | Stop a 1,5 ATR en 3 casos. | 3 cálculos. | **ATR** |
| 6.2 Medias (SMA, EMA) y cruces | Promedio y su retraso; EMA diaria de 365; cruce dorado y de la muerte. | Capas de medias y cruces del menú Herramientas. | Contar cuántos cruces dorados fallaron en 10 años. | 4 preguntas. | **EMA, SMA y cruces** |
| 6.3 RSI y VWAP | Qué miden y qué no: no son señales por sí mismos. | Capas RSI y VWAP. | — | 4 preguntas. | **RSI y VWAP** |
| 6.4 Herramientas que no puntúan | Fibonacci, golden pocket, puntos pivote diarios, spring/upthrust: se muestran, **no entran al puntaje** (`config/analysis.toml`). | Menú Herramientas completo. | — | 3 preguntas. | **Fibonacci, pivotes diarios, spring/upthrust, fondos, régimen del VIX** |
| Evidencia | Reglas simples parecían funcionar (Brock, Lakonishok y LeBaron, 1992) hasta corregir por haber probado muchas (Sullivan, Timmermann y White, 1999); patrones de velas sin valor en el Dow (Marshall, Young y Rose, 2006); revisión de Park e Irwin (2007). | — | — | — | — |

### Nivel 7 — Riesgo y tamaño de posición

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 7.1 R | Medir todo en múltiplos del riesgo: 1 R = lo que perdés si toca el stop. | Conversor puntos ↔ € ↔ R (motor). | 5 conversiones. | 5 cálculos. | — |
| 7.2 Stop obligatorio y su distancia | Stop siempre; entre 0,5 y 3 ATR (`[stop_distance]`); colchón por gaps de 0,25 ATR. | Ticket en modo preview: el motor rechaza un stop fuera de rango y dice por qué. | Ajustar 3 tickets hasta que pasen. | Replay: 3 entradas con stop válido. | — |
| 7.3 Tamaño | 1 % por operación, máximo 5 posiciones, 1 por instrumento, 1,5 % por grupo correlacionado (US500, US100, US30, US2000). | Calculadora de tamaño (`POST /api/v1/paper/preview`, ya existe). | 4 tamaños. | 4 cálculos. | **Ticket de paper** (preview y órdenes simuladas) |
| 7.4 Límites de la cuenta | Pérdida diaria 3 %, caída gradual (5 % → 1; 10 % → 0,5; 12,5 % → 0,25) y kill switch al 15 %. | Simulador de una mala racha: el motor muestra cómo se achica el tamaño. | — | 4 preguntas. | **Barra de estado de la operación** (kill switch, pérdida del día) |
| Caso real | Con los topes, la estrategia en uso pierde menos y más despacio, pero **sigue perdiendo**: el riesgo no arregla una estrategia sin ventaja (R039). | — | — | — | — |

### Nivel 8 — Expectativa, varianza y suerte ("ganarle a la varianza")

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 8.1 Expectativa | E = p × ganancia media − (1 − p) × pérdida media, en R y **después de costos**. | Deslizadores de p y de R; el motor calcula E. | 5 casos; encontrar el p mínimo para 1,5 R. | 4 cálculos. | — |
| 8.2 Rachas | Con 50 % de aciertos, 10 pérdidas seguidas en 1.000 operaciones es normal. | Generador de secuencias (motor, semilla fija). | Adivinar la racha más larga y comparar. | 3 preguntas. | — |
| 8.3 Monte Carlo | Muchos caminos posibles de la misma estrategia: caída máxima, racha, tiempo bajo el agua, riesgo de ruina (`research::montecarlo`: 10.000 caminos, bloques de 5, ruina = −30 %). | Abanico de caminos de una serie real del registro. | Leer p5, p50 y p95. | 4 preguntas. | **Visor de Monte Carlo** |
| 8.4 Suerte y habilidad | Un buen resultado no prueba una buena decisión (Duke, 2018; Taleb, 2001). Cuántas operaciones hacen falta para separar ventaja de ruido. | El mismo jugador con 100 y con 1.000 operaciones. | — | 4 preguntas. | — |
| Qué significa "ganarle a la varianza" | No se le gana: se la **sobrevive** con tamaño chico y se la **mide** con muestras grandes. Sin ventaja después de costos, ningún manejo de riesgo la crea (R039). | — | — | — | — |

### Nivel 9 — Backtesting y sus trampas

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 9.1 Mirar el futuro | Decidir al cierre de t, ejecutar en t+1; solo velas cerradas; sin repintado. | Un backtest "trucho" con look-ahead frente al correcto (motor, sobre los mismos datos). | Encontrar el error en 3 reglas escritas. | 4 preguntas. | **Corridas de backtest** (Performance) |
| 9.2 Sesgo de supervivencia | Probar con las acciones que existían ese día (ADR 0005, `universe`). | — | — | 3 preguntas. | — |
| 9.3 Sobreajuste y "muchas pruebas" | Cuantas más variantes probás, más probable es encontrar una buena por suerte: Sharpe deflactado (Bailey y López de Prado, 2014), PBO por CSCV (Bailey et al., 2017), y nuestro presupuesto global de pruebas (`dsr_trials`, hoy más de 350). | Matriz meses × variantes con su PBO (`research::pbo`). | Probar 20 variantes al azar y ver cuál "gana". | 4 preguntas. | **PBO** |
| 9.4 Walk-forward y fuera de muestra | Entrenar 24 meses, validar 3, avanzar 3; 12 meses que no se tocan (`config/data_split.toml`). | Reporte `walk_forward_report.v1` real. | — | 3 preguntas. | **Walk-forward** |
| 9.5 Casos reales | US100: 8 ventanas favorables (+0,07 %) frente a 51 ventanas con toda la historia (−6,1 %) (R006 → R007). Un error de código cambió una corrida (R034). El ML no mejoró a la referencia (R010–R013). | Línea de tiempo del registro. | Explicar cada caso en una frase. | 5 preguntas. | **Registro de resultados** |

### Nivel 10 — Tu propia estrategia: hipótesis, pre-registro, regímenes y noticias

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 10.1 De la idea a la hipótesis | Escribir una regla medible, con su placebo o su referencia (comprar y mantener, R018). | Plantilla de hipótesis. | Convertir 3 frases de "se ve como" en reglas. | El motor valida que la regla esté completa (entrada, stop, salida, costos, período). | — |
| 10.2 Pre-registro | Congelar la regla, los criterios y el período **antes** de ver resultados, con su hash (como `2026-11-programa-4-preregistro.md.sha256`). | Plantilla de pre-registro. | Pre-registrar una idea propia. | Revisión del motor: hash, criterios y N de pruebas. | **Laboratorio** (eje 3 de #707) |
| 10.3 Regímenes | El mismo sistema cambia con la volatilidad y la tendencia (`regime`, tercios de volatilidad, VIX). | Resultados cortados por régimen (ejemplo `diagnostico`). | — | 3 preguntas. | **Capa de régimen** |
| 10.4 Seguir la tendencia | La evidencia académica más larga (Moskowitz, Ooi y Pedersen, 2012; Hurst, Ooi y Pedersen, 2017) frente a nuestra prueba en tres índices, que **no pasó** (R016). | — | — | 3 preguntas. | — |
| 10.5 Noticias y macro | Una noticia influye desde su **hora de publicación real**; FOMC, CPI, empleo. Deriva antes del FOMC (Lucca y Moench, 2015) frente a nuestra H1 que **no pasó** (R027) y H2 (R031). | Calendario con hora real y replay de un día de CPI. | — | Replay en día de dato: el motor exige no entrar en la ventana prohibida (§7.2). | **Noticias y calendario** en la Terminal |
| 10.6 Caso Bruni | Leer un caso completo: ganó de verdad 3 campeonatos trimestrales (tabla oficial), pero sus backtests y sus pagos de prop firms **no están auditados**; sus tres reglas probadas con nuestro método **no pasaron** (R029, R030, R033). | Informe `docs/research/2026-11-gianluca-bruni.md`. | Separar lo verificado de lo no verificado. | 4 preguntas. | — |

### Nivel 11 — Avanzado, con advertencias: flujo de órdenes, perfil de volumen y GEX

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 11.1 Flujo de órdenes | Qué es el libro y el delta; **no tenemos libro**: nuestro `imbalance.v1` es una aproximación desde el precio, no flujo real. | Capa de desequilibrio aproximado, con el aviso. | — | 3 preguntas. | **Desequilibrio aproximado** |
| 11.2 Perfil de volumen | POC y área de valor desde las barras de 1 m del CME; caso H4.3 (R033, no pasó). | Perfil de una sesión real (motor, `ProfileTracker`). | — | 3 preguntas. | **Perfil de volumen** (descriptivo) |
| 11.3 GEX (exposición gamma) | La teoría (Barbon y Buraschi, 2020; Ni, Pearson, Poteshman y White, 2021) y por qué **no la podemos medir**: no tenemos datos de opciones; sumarlos pide un ADR. | Solo diagrama. | — | 3 preguntas. | — |

### Nivel 12 — Cuentas fondeadas (prop firms)

| Lección | Objetivo | Interactivo | Ejercicio | Examen | Desbloquea |
|---|---|---|---|---|---|
| 12.1 Cómo funcionan | Fases, objetivo de ganancia, pérdida diaria, caída máxima fija o que sigue al máximo, días mínimos, reparto, pago de la evaluación. | Reglas de una plantilla genérica (§6). | Leer 3 reglamentos y encontrar la regla más peligrosa. | 5 preguntas. | — |
| 12.2 Quién gana con el negocio | Ingresos de la firma = cuotas de los que fallan + lo que retiene de los que pasan. Cuánto pagás en promedio hasta pasar. | Simulador: costo esperado en cuotas. | Calcular el valor esperado de un programa. | 3 cálculos. | — |
| 12.3 La evidencia honesta | No hay tasas de aprobación auditadas públicas; lo que publican las firmas es autodeclarado. Evidencia académica sobre traders intradía minoristas (Barber, Lee, Liu y Odean, 2014; Chague, De-Losso y Giovannetti, 2019). | Tabla de fuentes con su nivel de prueba. | — | 4 preguntas. | — |
| 12.4 Pasar por suerte | Una estrategia con E = 0 después de costos igual pasa a veces: el simulador lo muestra. | Simulador con operaciones al azar frente a tus operaciones. | Comparar tu probabilidad con la del azar. | Simulación con tus operaciones de replay. | **Simulador de cuentas fondeadas** |

### Mapa de desbloqueos (resumen)

| Nivel | Herramientas que se habilitan |
|---|---|
| 0 | Watchlist, panel del instrumento, Glosario |
| 1 | Selector de fuente (CFD / futuros) |
| 2 | Costos en el ticket, calculadora de costos |
| 3 | Gráfico de velas, timeframes, Multigráfico, replay libre, regla, notas del Diario |
| 4 | Pivotes del motor, dibujo, soportes y resistencias, patrones chartistas |
| 5 | Mapa de liquidez, sweep y estructura, FVG y order blocks, estadísticas |
| 6 | ATR, medias y cruces, RSI, VWAP, Fibonacci y el resto del menú Herramientas |
| 7 | Ticket de paper, barra de estado de la operación |
| 8 | Visor de Monte Carlo |
| 9 | Corridas de backtest, PBO, walk-forward, registro de resultados |
| 10 | Laboratorio, capa de régimen, Noticias y calendario en la Terminal |
| 11 | Desequilibrio aproximado, perfil de volumen |
| 12 | Simulador de cuentas fondeadas |

## 4. Sistema de desbloqueo

### 4.1 Cómo funciona

- Cada herramienta de la UI tiene un **id estable** (por ejemplo `tool.fvg`, `tool.ticket`,
  `tool.montecarlo`). El catálogo de lecciones, sus exámenes y qué id desbloquea cada una van en
  config (`config/academy.toml`, propuesto; nada hardcodeado, CLAUDE.md §5b).
- Mientras está bloqueada, la herramienta **se ve** (gris, con un candado) y al tocarla dice qué
  lección la abre y lleva ahí. No se esconde: ver lo que viene motiva.
- **Ajustes → Academia → "Desbloquear todo" o por herramienta**: el dueño puede saltar el
  candado. Queda registrado como un evento más (quién, cuándo, qué) y la herramienta muestra
  "desbloqueada a mano". Se puede volver a bloquear.
- El desbloqueo no cambia **nada** del motor, del riesgo ni del paper: solo qué se ve en la UI.
  El kill switch, los límites y la simulación siguen iguales con todo desbloqueado.

### 4.2 Dónde se guarda el progreso (propuesta)

Se evaluaron tres lugares:

| Opción | A favor | En contra |
|---|---|---|
| `localStorage` del navegador | Fácil, sin backend. | Se pierde al borrar datos; no sigue al celular (PWA); la nota de un examen la calcula el motor, así que igual hay que guardarla del lado del motor; fácil de editar a mano. |
| Archivo local en el Mac mini | Simple. | Fuera de PostgreSQL, sin el patrón de solo agregar ni migraciones. |
| **PostgreSQL local (Mac mini), por el gateway** | Un solo lugar para la computadora y el celular; el motor escribe la nota del examen junto al intento; historial de solo agregar como el diario y el fondo. | Pide rutas nuevas de escritura en el gateway. |

**Propuesta: PostgreSQL local, solo agregar**, con un módulo nuevo del motor (`academy`, ver §8).

- Tabla `academy_events` (propuesta): `lesson_completed`, `exam_attempt` (con su nota y el
  `exam_result` del motor), `tool_override` (desbloqueo o rebloqueo a mano). Nunca se edita una
  fila: el estado se reconstruye leyendo los eventos (como `fund_movements` y `replay` del fondo).
- El motor calcula el **estado** (lecciones aprobadas y herramientas desbloqueadas) y lo sirve como
  `academy_progress.v1`. La UI no decide si algo se aprobó.
- `localStorage` queda solo para preferencias de pantalla (lección abierta, pestaña, tamaño de
  letra), como hoy.
- Las rutas de escritura usan el mismo token local anti-CSRF que el broker simulado
  (`POST /api/v1/paper/orders`). Como el gateway hoy es "de solo lectura salvo paper", el PR que las
  sume actualiza su ficha (`docs/modules/gateway.md`).
- Se guarda con la persona (`member`), pensando en que algún día haya más miembros (spec §1.2);
  hoy es solo el dueño.

## 5. Exámenes prácticos con replay

### 5.1 Qué ve el estudiante

1. Elige el tipo de examen (niveles, entradas, día de dato...). El motor elige **instrumento,
   timeframe y fecha al azar** (con semilla guardada para poder repetir el mismo examen).
2. La fecha **está oculta**: el eje dice "día 1, 09:30" en lugar de "12-03-2019". El precio es el
   real (para que los números redondos sigan siendo redondos). **Opcional (PENDIENTE)**: mover el
   precio para que no se reconozca el año; apagado por defecto porque cambia los redondos.
3. Ve el calentamiento (por ejemplo 300 velas, **PENDIENTE**) y avanza vela por vela, o de a N.
4. Marca **niveles** (línea con precio), **entradas** (dirección, precio o "a mercado en la
   siguiente"), **stop** y **objetivo**, y escribe el **plan** antes y la **nota** después.
5. Al terminar ve el resultado: primero la **nota de proceso**, después el R (separados a
   propósito), y al final se revela la fecha real.

### 5.2 Datos

- **Futuros de Databento de 1 minuto** (ES, NQ, YM, FDAX, desde 2011, `futures_bars_1m`),
  agrupados a 15 m, 1 h y 1 d **en Rust** (`cfds_market_data::futures`), con los vencimientos
  marcados.
- **Velas del CFD de IG** (`candles`) e **IBKR** (`ibkr_bar.v1`) cuando haya historia suficiente.
- Costos siempre del catálogo (`InstrumentCosts::from_spec`): el replay del CFD paga spread,
  slippage y financiación como el backtest.

### 5.3 Reglas para no mirar el futuro

- El **cursor vive en el motor**. La UI pide "avanzar" y el motor manda solo las velas hasta el
  cursor. No hay forma de pedir una vela posterior (la ruta la rechaza).
- Indicadores y capas del motor se calculan **con los datos hasta el cursor** (las mismas
  funciones puras de siempre, sin repintado).
- Una orden marcada en la vela `t` se ejecuta en la apertura de `t+1` (CLAUDE.md §5b).
- El sorteo de fechas sale **solo del período de investigación**: nunca de los 12 meses del fuera
  de muestra (`config/data_split.toml`), con la misma guardia que usa la investigación
  (`research_period`). Así el dueño no "aprende" el período que después tiene que juzgar una
  estrategia suya.
- Se excluyen días con datos rotos (`data_validation`) y se puede pedir o excluir días de dato
  (calendario oficial con `published_at`).

### 5.4 Cómo califica el motor

Todo en Rust, con funciones puras y config (`config/academy.toml`, umbrales **PENDIENTES**):

| Tipo | Qué se mide | Cómo |
|---|---|---|
| **Niveles** | Si tus niveles coinciden con los objetivos del motor. | Precisión y cobertura contra `liquidity_level.v1` y los soportes y resistencias de `patterns` calculados **hasta el cursor**, con tolerancia en ATR (propuesta 0,25 ATR, PENDIENTE). |
| **Pivotes y eventos** | Si marcaste pivotes, barridas, FVG en la vela correcta. | Comparación con los eventos del motor confirmados hasta el cursor; marcar antes de la confirmación cuenta como "adelantado" (y se explica por qué). |
| **Operación** | Proceso: stop presente y en rango (0,5–3 ATR), tamaño ≤ 1 % (o el de la lección), plan escrito antes, sin entrar en ventana de noticia, costos aceptados. | `cfds_risk::check` y las salidas del backtest (stop, objetivo, cierre forzado, gaps), con costos del catálogo. |
| **Resultado** | R neto, costos en R, MAE/MFE. | Se muestra, **no aprueba ni reprueba**. |

La nota final = cumplimiento de reglas (sí/no por regla) + precisión en niveles y eventos. Un
examen de operaciones necesita varias operaciones (propuesta: 10, PENDIENTE) para que una sola no
decida.

### 5.5 Rutas y contratos que hacen falta (solo nombres; se definen después, contratos primero)

| Contrato (propuesto) | Para qué |
|---|---|
| `academy_catalog.v1` | Niveles, lecciones, exámenes y el id de la herramienta que desbloquea cada uno (sin las claves). |
| `academy_event.v1` | Un evento de progreso (lección, intento, desbloqueo a mano). |
| `academy_progress.v1` | Estado calculado por el motor: aprobadas, herramientas abiertas, a mano o no. |
| `quiz_answer.v1` / `quiz_result.v1` | Respuesta a preguntas y su corrección del motor. |
| `replay_session.v1` | Sesión: id, instrumento, timeframe, fuente, semilla, calentamiento; la fecha oculta. |
| `replay_step.v1` | Velas nuevas hasta el cursor, con las capas del motor calculadas hasta ahí. |
| `replay_mark.v1` | Nivel, pivote o evento que marca el estudiante, con el cursor del momento. |
| `replay_order.v1` | Entrada con dirección, stop, objetivo y tamaño, con el cursor del momento. |
| `exam_result.v1` | Nota de proceso por regla, precisión de niveles, R y costos, y la fecha revelada. |

Rutas del gateway (propuestas, todas bajo `/api/v1/academy/`): `GET catalog`, `GET progress`,
`POST events` (desbloqueo a mano), `POST quiz/{id}`, `POST replay` (crear), `POST replay/{id}/step`,
`POST replay/{id}/marks`, `POST replay/{id}/orders`, `POST replay/{id}/finish`.

## 6. Simulador de cuentas fondeadas

### 6.1 Reglas configurables (`prop_rules.v1`, propuesto)

- Fases (evaluación 1, 2, fondeada) con su **objetivo de ganancia**.
- **Pérdida diaria máxima**: desde el saldo o el capital al inicio del día, con la **zona horaria
  de corte de la firma**.
- **Caída máxima**: fija desde el saldo inicial, que sigue al máximo **al cierre del día** o que
  sigue al máximo **intradía**, y si se "traba" en el saldo inicial.
- **Días mínimos y máximos** de operación.
- **Regla de consistencia** (el mejor día no puede ser más del X % del total).
- Restricciones: noticias, fin de semana, tamaño o apalancamiento máximo.
- Economía: **cuota**, si se devuelve, **reparto** de ganancias, frecuencia de pagos.

**No se inventan reglas de firmas reales** (CLAUDE.md §5b). El simulador trae una **plantilla
genérica** con todos los valores vacíos. Para cargar una firma real hace falta su reglamento
publicado con link y fecha, como las fuentes de `config/instruments.toml`.

### 6.2 Qué calcula el motor

1. **Recorrido determinista**: las operaciones (de un examen de replay, del paper o de una corrida
   de backtest con su `run_id`) pasan por las reglas en orden. Resultado: pasó, falló (qué regla y
   cuándo) o sigue.
   - La caída intradía necesita el capital **vela por vela** (no solo al cierre de la operación).
     El backtest de cartera hoy guarda la curva diaria; para la regla intradía hace falta el capital
     a mercado en cada vela (ya se calcula para `mark_peak`, falta exponerlo). Mientras no esté,
     esa regla se marca "no simulable" en vez de aproximarla.
2. **Monte Carlo**: remuestrea las operaciones (o los días, en bloques) muchas veces, reusando
   `research::montecarlo` (SplitMix64, semilla fija; no se escribe otro bootstrap). Da:
   - probabilidad de pasar cada fase y de llegar a cobrar, con su intervalo;
   - días esperados hasta pasar o fallar;
   - cuotas esperadas pagadas hasta pasar;
   - **valor esperado del programa** = P(cobrar) × cobro esperado − cuotas esperadas.
3. **Comparación con el azar**: la misma cantidad de operaciones con E = 0 después de costos.
   Si tu probabilidad no es claramente mayor que la del azar, la pantalla lo dice.

### 6.3 Lo que la pantalla nunca dice

- Nada de "vas a pasar" ni "vas a ganar". Siempre: "con estas operaciones y estas reglas, en el
  pasado simulado, X de cada 100 caminos pasaron (intervalo ...)".
- El aviso fijo: resultados simulados con datos pasados; las firmas cambian sus reglas; los pagos
  dependen de la firma.
- Pagar una evaluación real es dinero real: **queda fuera de la app** (CLAUDE.md §5 y fase 11).

## 7. Fuentes por nivel

Regla: libros y artículos con datos, revisados o con método reproducible. **Sin influencers ni
cursos pagos.** Cada fuente lleva su nivel de prueba: **A** = artículo revisado o dato oficial,
**B** = libro de un profesional o académico con método explícito, **C** = verificado en parte, con
afirmaciones sin auditar. Antes de citar una cifra en una lección, el PR de esa lección la verifica
contra la fuente original.

| Nivel | Fuentes |
|---|---|
| 0 | ESMA, medidas de intervención sobre CFD (2018; decisión 2018/796, ya en `config/instruments.toml`) — A. Barber y Odean (2000), *Trading Is Hazardous to Your Wealth*, J. of Finance — A. |
| 1 | Hull, *Options, Futures, and Other Derivatives* — B. Harris (2003), *Trading and Exchanges* — B. |
| 2 | Harris (2003), costos de transacción — B. Reglamentos de costos de IG (fuentes de `config/instruments.toml`) — A. Nuestro registro R001–R003, R017, R021, R026 — A (propio, reproducible). |
| 3 | Harris (2003) — B. |
| 4 | Osler (2000), *Support for Resistance*, FRBNY Economic Policy Review — A. Osler (2003), *Currency Orders and Exchange Rate Dynamics*, J. of Finance — A. Lo, Mamaysky y Wang (2000), *Foundations of Technical Analysis*, J. of Finance — A. Registro R020 — A. |
| 5 | Registro R034–R038 (FVG frente a placebo) — A. Sobre ICT/SMC no hay estudios revisados: se enseña como **definición** del motor, no como ventaja probada. |
| 6 | Wilder (1978), *New Concepts in Technical Trading Systems* (origen de ATR y RSI) — B. Brock, Lakonishok y LeBaron (1992), J. of Finance — A. Sullivan, Timmermann y White (1999), J. of Finance — A. Marshall, Young y Rose (2006), J. of Banking & Finance — A. Park e Irwin (2007), J. of Economic Surveys — A. Aronson (2006), *Evidence-Based Technical Analysis* — B. |
| 7 | Kelly (1956) — A. Thorp (2006), *The Kelly Criterion in Blackjack, Sports Betting and the Stock Market* — A/B. Registro R039 — A. |
| 8 | Efron y Tibshirani (1993), *An Introduction to the Bootstrap* — B. Politis y Romano (1992), bootstrap por bloques — A. Duke (2018), *Thinking in Bets* — B. Taleb (2001), *Fooled by Randomness* — B. |
| 9 | Bailey y López de Prado (2014), *The Deflated Sharpe Ratio*, J. of Portfolio Management — A. Bailey, Borwein, López de Prado y Zhu (2017), *The Probability of Backtest Overfitting*, J. of Computational Finance — A. Harvey, Liu y Zhu (2016), *…and the Cross-Section of Expected Returns*, RFS — A. White (2000), *A Reality Check for Data Snooping*, Econometrica — A. López de Prado (2018), *Advances in Financial Machine Learning* — B. Registro R006, R007, R010–R013, R034 — A. |
| 10 | Harvey (2017), *The Scientific Outlook in Financial Economics*, J. of Finance (pre-registro y p-hacking) — A. Moskowitz, Ooi y Pedersen (2012), *Time Series Momentum*, JFE — A. Hurst, Ooi y Pedersen (2017), *A Century of Evidence on Trend-Following Investing* — B. Moreira y Muir (2017), *Volatility-Managed Portfolios*, J. of Finance — A. Ang y Timmermann (2012), *Regime Changes and Financial Markets* — A. Lucca y Moench (2015), *The Pre-FOMC Announcement Drift*, J. of Finance — A. Andersen, Bollerslev, Diebold y Vega (2003), *Micro Effects of Macro Announcements*, AER — A. Registro R016, R019, R027, R031 — A. **Gianluca Bruni**: resultados de competencia **verificados** en la tabla oficial de la WCC, pero backtests, pagos de prop firms y métricas **sin auditar**; conflictos de interés declarados (`docs/research/2026-11-gianluca-bruni.md`) — **C**. Sus reglas probadas por nosotros: R029, R030, R033 — A. |
| 11 | O'Hara (1995), *Market Microstructure Theory* — B. Barbon y Buraschi (2020), *Gamma Fragility* (documento de trabajo) — B. Ni, Pearson, Poteshman y White (2021), RFS — A. Registro R033 — A. |
| 12 | Barber, Lee, Liu y Odean (2014), *The Cross-Section of Speculator Skill: Evidence from Day Trading*, J. of Financial Markets — A. Chague, De-Losso y Giovannetti (2019), *Day Trading for a Living?* (documento de trabajo, datos completos de Brasil) — B. Estadísticas que publican las firmas — C (autodeclaradas). Acciones de reguladores contra firmas de fondeo: se citan solo con el expediente oficial leído y fechado (pendiente de un Issue de investigación). |
| Transversal (psicología y diario) | Kahneman y Tversky (1979), *Prospect Theory*, Econometrica — A. Shefrin y Statman (1985), efecto disposición, J. of Finance — A. Odean (1998), *Are Investors Reluctant to Realize Their Losses?*, J. of Finance — A. Tetlock y Gardner (2015), *Superforecasting* (calibración) — B. |

**Lo que no entra:** videos, cursos y cuentas de redes sin resultados auditados, aunque sean
populares. Si el dueño trae un video, se analiza con el método de
`docs/research/videos/` (verificado / no verificado) y, si vale, se pre-registra y se prueba.

## 8. Plan de implementación (PRs chicos)

Módulos nuevos propuestos (cada uno con su ficha en `docs/modules/` y su fila en el registro, en el
PR que lo crea):

- **`academy`** (Rust, crate `cfds-academy`): catálogo validado al arrancar, progreso desde
  eventos de solo agregar, sesiones de replay con el cursor del lado del motor y calificación de
  exámenes. Reusa `risk`, `backtest` (salidas y costos), `liquidity`, `patterns`, `indicators` y
  `market-data`; no reimplementa ningún cálculo.
- **`propsim`** (Rust, crate `cfds-propsim`): reglas de cuentas fondeadas como funciones puras y
  su Monte Carlo, reusando `research::montecarlo`.

| # | PR | Qué | Depende de |
|---|---|---|---|
| 0 | **Este** | Diseño y temario. | — |
| 1 | Contratos de progreso | `academy_catalog.v1`, `academy_event.v1`, `academy_progress.v1`, `quiz_answer.v1`, `quiz_result.v1` + `config/academy.toml` con los niveles 0–2. | Aprobación de este diseño |
| 2 | Motor: `cfds-academy` base | Catálogo, validación, migración `academy_events`, estado desde eventos, corrección de preguntas. | 1 |
| 3 | Gateway | Rutas `catalog`, `progress`, `events`, `quiz` con token anti-CSRF; ficha del gateway. | 2 |
| 4 | UI: vista Academia | `#academia` con niveles y lecciones (Markdown de `docs/academia/`), estado del motor. | 3 |
| 5 | UI: candados | Componente que envuelve cada herramienta con su id; "Desbloquear" en Ajustes. | 3, Ajustes (#647) |
| 6 | Contenido niveles 0–2 | Texto, preguntas y claves (en config del motor). | 1 |
| 7 | Contratos de replay | `replay_session.v1`, `replay_step.v1`, `replay_mark.v1`, `replay_order.v1`, `exam_result.v1`. | 2 |
| 8 | Motor: replay | Sesión con semilla, fecha oculta, cursor, sorteo solo en el período de investigación; capas hasta el cursor. | 7 |
| 9 | Motor: calificación | Niveles y eventos contra el motor; operaciones con `risk` y las salidas del backtest. Tests de no mirar el futuro (pedir una vela después del cursor falla) y de sin repintado. | 8 |
| 10 | Gateway + UI replay | Rutas de replay; vista de examen con `LiquidityChart`, dibujo y ticket. | 9 |
| 11 | Contenido niveles 3–5 | Un PR por nivel. | 10 |
| 12 | Contratos y motor de `propsim` | `prop_rules.v1`, `prop_sim_request.v1`, `prop_sim_result.v1`; recorrido determinista con tests de cada regla (borde: pérdida diaria justo en el límite, caída que sigue al máximo intradía, días mínimos). | 2 |
| 13 | Capital por vela | Exponer el capital a mercado vela por vela del backtest para la regla intradía. | 12 |
| 14 | Monte Carlo de `propsim` + UI del simulador | Reusa `research::montecarlo`; pantalla con la comparación contra el azar. | 12, 13 |
| 15 | Contenido niveles 6–12 | Un PR por nivel. | 10, 14 |

Cada PR de motor que corra un examen o una simulación sobre datos reales como **prueba de
investigación** agrega su fila al registro de resultados; los exámenes del estudiante **no** son
pruebas de investigación y no suman al N de `dsr_trials`.

### Qué se puede reusar hoy

- **Gráficos**: `LiquidityChart` con capas, dibujo, regla, OHLC bajo el cursor, Multigráfico y
  cursor sincronizado.
- **Capas del motor**: niveles de liquidez, sweeps, FVG, order blocks, BOS/CHoCH, pivotes, soportes
  y resistencias, estadísticas con Wilson, bloque `analysis` (EMA diaria, Fibonacci, pivotes
  diarios, spring/upthrust, fondos, cruces, VIX).
- **Riesgo y costos**: `POST /api/v1/paper/preview` ya devuelve el veredicto del ticket con tamaño,
  margen y costos; `cfds_risk::check`; `InstrumentCosts::from_spec`.
- **Investigación**: `research::montecarlo`, `research::pbo`, Sharpe deflactado, walk-forward,
  diagnóstico por cortes, el registro de resultados y los informes de `docs/analysis/`.
- **Datos**: futuros de 1 m de Databento desde 2011, velas de IG e IBKR, calendario oficial con
  `published_at`, tipos de referencia, VIX.
- **UI**: Glosario interactivo (patrón para el Markdown de las lecciones), Diario, Trofeos de
  proceso, paleta de comandos, Ajustes.

## 9. Riesgos y cómo se cuidan

| Riesgo | Cuidado |
|---|---|
| Que la Academia parezca prometer ganancias. | Textos revisados contra CLAUDE.md §5; la nota es de proceso; cada caso real muestra también lo que no funcionó. |
| Que la UI empiece a calcular "para mostrar rápido". | Toda nota y todo número de trading viene del motor; los tests de la UI usan fixtures de `contracts/`. |
| Mirar el futuro en el replay. | Cursor en el motor, ruta que rechaza velas posteriores, tests que lo prueban. |
| Contaminar el fuera de muestra. | Sorteo solo en el período de investigación, con la guardia existente. |
| Inventar reglas de prop firms o cifras de aprobación. | Plantilla vacía; firma real solo con reglamento citado; cifras solo con fuente leída. |
| Demasiado contenido para mantener. | Un PR por nivel; las cifras salen de config y del registro, no del texto. |

## 10. Decisiones que necesita el dueño

1. ¿Aprobás el temario de 13 niveles (0–12), o querés sacar o mover alguno?
2. ¿Guardar el progreso en **PostgreSQL local por el gateway** (propuesta) en lugar de
   `localStorage`?
3. Umbrales PENDIENTES: aprobación de preguntas (80 %), tolerancia de niveles (0,25 ATR),
   operaciones por examen (10), calentamiento del replay (300 velas).
4. ¿Mover el precio en el replay para que no se reconozca el año? (propuesta: no).
5. ¿Herramientas bloqueadas visibles con candado (propuesta) o escondidas?
6. ¿Arrancamos por los PRs 1 a 6 (progreso y niveles 0–2) antes que el replay?
