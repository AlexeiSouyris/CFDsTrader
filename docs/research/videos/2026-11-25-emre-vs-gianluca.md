# Video: ex-gestor de hedge fund contra campeón de trading minorista (Emre y Gianluca Bruni)

- **Link:** https://youtu.be/2WZ137MtCiw
- **Canal:** IQCapital. Es una **empresa de fondeo (prop firm)**: el video termina con publicidad de sus desafíos de 9 USD. Eso es un sesgo a tener en cuenta (ver §4).
- **Publicado:** 30-10-2026. **Duración:** 55 min.
- **Analizado:** 25-11-2026, con la transcripción automática en inglés. Pedido del dueño: mirar sobre todo las estrategias de Gianluca y qué podemos aprender o usar.

## 1. Qué muestra

Dos traders operan la misma sesión en vivo, durante unas 3 horas:

- **Emre:** discrecional (decide en el momento). Opera opciones de ES (S&P 500) apostando a que el precio se quede dentro de un rango de 100 puntos, más futuros para cubrirse. Usa el flujo de órdenes (footprint, delta) y los niveles de exposición gamma (GEX). Los 20.000 USD de ganancia que muestra son de una posición abierta **una semana antes** del video, sin realizar (el video lo aclara).
- **Gianluca:** mecánico (reglas fijas). Opera momentum en el futuro del Nasdaq (NQ) y termina la sesión con unos +2.100 USD: una operación perdida, una ganada con piramidación y una más pequeña.

## 2. La estrategia de Gianluca, tal como la explica

**a) Estructura (de qué lado está el mercado).** Una regla objetiva, sin interpretar el gráfico:
- **Vela válida:** el cuerpo es más largo que la mecha más larga.
- **Zona** (consolidación, oferta o demanda): un tramo con al menos **3 velas válidas**.
- **Ruptura válida:** el cuerpo de una vela cierra fuera de la zona. Si una vela solo la perfora con la mecha, no cuenta.
- Lee dos capas: la estructura **actual** (la zona más reciente) y la **periférica** (la zona anterior más grande). Prefiere operar cuando las dos apuntan para el mismo lado.
- Usa el perfil de volumen (área de valor) solo como contexto y como objetivo.

**b) Disparador (momentum):**
- **dos velas seguidas** en la dirección de la estructura, cada una con un cuerpo de al menos **60 ticks**, es decir 15 puntos en NQ;
- su marco de tiempo normal es de **15 minutos**. En el video bajó a 5 minutos para tener más operaciones en pocas horas.

**c) Salida:**
- **stop** del otro lado de la primera vela de momentum;
- **objetivo fijo de 1,5 veces el riesgo** (1:1,5);
- la posición se calcula para arriesgar un monto fijo (1.000 USD en el video).

**d) Piramidación:** si aparece otra vela de momentum válida a favor, suma contratos y sube el stop de toda la posición.

**e) Lo que dice de sus números:**
- expectativa de **0,12 R** por operación;
- un test de suerte: entradas al azar contra su estrategia;
- 16 % de chances de ir perdiendo después de 100 operaciones, y 2,3 % después de 406.

Dice dos cosas importantes. Esta estrategia **no es la principal** y está pensada para **empresas de fondeo**, donde importa operar muchas veces. Y probablemente **no le gana a un índice comprado y mantenido** con una cuenta propia, así que con plata propia no habría que operarla.

## 3. Afirmaciones comprobables

| Afirmación | Veredicto |
|---|---|
| "Con 10 operaciones, la tasa de aciertos puede variar mucho por suerte; con 100 o más se estabiliza" (usa 1/√N) | **Cierto en lo esencial.** El error estándar de una tasa de aciertos es √(p(1−p)/N). Con p = 0,6 y N = 10 es ±15,5 %; su 1/√N (31 %) se parece a un intervalo de ±2 errores estándar. Con N = 1.000 da 57–63 %, que coincide con un intervalo del 95 %. |
| 0,12 R de expectativa, 16 % de perder tras 100 operaciones y 2,3 % tras 406 | **Coherente.** Con stop de 1 R y objetivo de 1,5 R, 0,12 R implica acertar el 44,8 %. El desvío por operación es de 1,24 R. Tras 100 operaciones se espera +12 R con desvío de 12,4 R, y P(pérdida) ≈ 16,6 %. Tras 406 se espera +48,7 R con desvío de 25 R, y P(pérdida) ≈ 2,6 %. Sus números cierran. |
| "Si hacés 1.000 de estas operaciones, ganás plata" | **Solo si la ventaja es real y no se desgasta.** Emre lo marca en el video: supone que la tasa de aciertos no cambia con el tiempo. No mostró resultados fuera de muestra ni walk-forward, y él mismo dice que tiene pocas muestras. |
| Emre: "el mercado se queda en rango la mayor parte del tiempo, por la distribución gaussiana", y "30–35 % del tiempo es direccional" | **Opinión o folklore.** Los retornos de los índices **no son gaussianos**: tienen colas gruesas. Lo de 30–35 % no tiene fuente. |
| Emre: "las competencias de trading no reflejan la habilidad: es un solo dato" | **Cierto.** Es lo mismo que pide nuestra metodología: muchas muestras, no un día. |

**Costos:** nadie habla de comisiones ni de slippage. Con una expectativa de 0,12 R, los costos pueden comerse buena parte de la ventaja (ver §5).

## 4. Opinión, venta y sesgos

- **El video es publicidad de una empresa de fondeo.** Su mensaje ("es mecánico, a la larga gana") empuja a comprar desafíos. El propio Gianluca dice que la estrategia está hecha **para el modelo de pagos de las empresas de fondeo**: el trader arriesga el precio del desafío, no su capital, y eso cambia qué estrategia conviene. **No sirve igual para una cuenta propia.**
- **Se contradice durante la sesión:**
  - su sistema está probado en 15 minutos, pero opera en 5;
  - toma una operación que él mismo llama "no es el mejor setup" porque "hay que operar".
  Sirve para mostrar en un video, pero no es lo que dice su prueba.
- **Lo de Emre no se puede copiar:** es discrecional, usa opciones y datos pagos, y la ganancia mostrada no se hizo en la sesión.

## 5. Qué podemos aprender y usar

**Lo que ya hacemos y el video confirma:**
- **reglas mecánicas y objetivas** en lugar de "se ve como" (CLAUDE.md §5b);
- **pensar en cientos de operaciones**, no en un día;
- **compararse contra comprar y mantener.** Él admite que su estrategia probablemente no le gana; nosotros lo exigimos como criterio de aprobación.

**Ideas nuevas que valen:**
1. **Su definición de vela válida y de zona** (cuerpo mayor que la mecha más larga, zona con 3 velas válidas, ruptura con cierre de cuerpo) es objetiva y se puede implementar como función pura en Rust (`patterns`). Sirve como definición medible de "consolidación" y de "ruptura".
2. **El test de suerte** (entradas al azar con el mismo stop, objetivo y cantidad de operaciones) es un control sencillo y fácil de entender. Hoy usamos p-valor y Sharpe deflactado. Agregarlo como **reporte informativo** cuesta poco y ayuda a explicar resultados.
3. **Una hipótesis para probar en nuestro nicho (US100):** momentum de dos velas de 15 minutos a favor de la estructura, con stop detrás de la primera vela y objetivo de 1,5 R. Hay datos para hacerlo: **NQ en velas de 1 minuto desde 2011**, ya cargado (Databento), para armar velas de 15 minutos.

**Cuidados si se prueba** (todo se fija antes de mirar resultados):
- **"60 ticks" no escala:** en 2013 el NQ estaba cerca de 2.800 y hoy cerca de 26.000, así que 15 puntos son mucho más chicos hoy. Hay que congelar antes **la regla literal** (15 puntos, que solo tiene sentido en los años recientes) **y una versión normalizada** (por ejemplo, el cuerpo en múltiplos del ATR de 15 minutos). Son **dos pruebas**, y las dos cuentan para el registro.
- **Costos del CFD US100:** spread de IG, slippage y financiación si cruza la noche. Con stops de 20–40 puntos, un costo de 1–2 puntos por operación ya es 0,05 R o más, casi la mitad de 0,12 R.
- **Piramidación:** hay que definirla con números y que pase por el módulo de riesgo (1 % por operación, pérdida máxima diaria). Si no, se prueba sin piramidar.
- **Presupuesto de error:** el programa del Nasdaq ya usó su 2,5 % (H1 y H2). Una prueba nueva necesita una **pre-registración nueva con presupuesto propio**, y eso lo decide el dueño.

## 6. Conclusión

Es buen material **de proceso**: disciplina, reglas objetivas y tamaño de muestra. Sus números son coherentes entre sí. **No es evidencia de una ventaja:** no hay prueba fuera de muestra, no hay costos, opera fuera de su marco probado, y el propio autor dice que con plata propia probablemente no le gana al índice. Vale la pena probar la idea (3) en US100 **con nuestra metodología completa**, sabiendo que lo más probable es que, con los costos del CFD, no pase.
