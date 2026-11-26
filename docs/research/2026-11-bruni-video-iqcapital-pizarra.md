# Bruni en IQCapital (02-10-2026): lo que dibuja en la pizarra y muestra en los gráficos

- **Issue:** #855. **Escrito:** 26-11-2026 por un agente de IA (Claude Code) para el dueño.
- **Video:** «IQCapital, 02-10-2026», 1 h 43 min, https://www.youtube.com/watch?v=nlLGlVJGrtw. IQCapital es una **empresa de fondeo (prop firm)** y el video tiene dos publicidades de sus desafíos de 9 USD.
- **Cómo se hizo:** se miraron los fotogramas del video (uno cada 20 s y uno en cada cambio de plano; 643 después de sacar los repetidos) y los cuadros de la pizarra, el gráfico y el software a resolución completa. Todo se cruzó con los subtítulos automáticos en inglés. **No se suben imágenes al repo** (derechos de autor): se describen y se cita el minuto.
- **Qué agrega:** §3.C y §3.D de `2026-11-gianluca-bruni.md` ya salieron de este video, por el audio. Este documento agrega **lo que se ve**: lo que escribe, lo que dibuja, lo que mide en TradingView y lo que muestra su software. Lo que ya está allá no se repite: se enlaza.

Marcas: **[dice]** lo dice él; **[muestra]** está escrito o se ve en pantalla; **[inferido]** es interpretación nuestra.
Nada de esto es una recomendación de operar. Son resultados pasados de otra persona, que no se pueden auditar y que no prometen nada.

## 1. Resumen en diez líneas (la estrategia tal como la dibuja)

1. **Un solo marco de tiempo: 15 minutos**, en el futuro del Nasdaq (NQ). Escribe «15 TF» arriba del dibujo [muestra].
2. **Tendencia:** dibuja dos perfiles de volumen por sesión. El de la sesión actual queda **más arriba** que el de la anterior (área de valor y POC más altos) y eso, para él, es «tendencia alcista» [muestra][dice].
3. **Estructura:** el mínimo de la estructura es el mínimo de la **vela bajista válida más cercana** a la ruptura (cuerpo más grande que la mecha más larga). Si el precio lo perfora, se anula todo [muestra][dice].
4. **Espera:** que el precio vuelva al **área de valor de la sesión anterior**. Alcanza con tocarla, aunque sea con una mecha [muestra en el gráfico][dice].
5. **Disparador, todo en la misma vela:** el cuerpo cierra por encima de la mecha más alta de la vela anterior **y** cierra por encima del borde de arriba de esa área (VAH), las dos cosas con un margen mínimo («0,6» en forex, «6» en el Nasdaq) [muestra][dice].
6. **Volumen «eficiente»:** el volumen de esa vela es **menor que el de la anterior** o **menor que su media de 20** («poco esfuerzo, mucho resultado») [muestra][dice].
7. **Orden:** entrada al cierre, stop en el mínimo de la vela y objetivo de **2 veces el riesgo** [dice]. En el ejemplo, la herramienta de posición marca R:R 1,98 [muestra].
8. **Reentradas:** después de la primera operación repite con la misma vela y el mismo volumen, sin pedir de nuevo la ruptura del VAH, hasta el **máximo de la estructura**. Ahí cierra todo [muestra][dice].
9. **Tamaño:** cinco preguntas de sí o no (régimen, racha, frescura, tipo de volumen, sesión de Nueva York) le dan de 0 a 5 puntos. Con 5/5 arriesga ½ Kelly, con 4/5 ¼, con 3/5 ⅕ y con 2/5 ⅙ [muestra].
10. **Lo que no se puede verificar:** las cifras (147 R, Sharpe 2,2, 7,9 de autocorrelación) salen de un backtest que **él cargó a mano** en un software pago, que muestra conectado con su usuario. En nuestros datos, esta regla **ya se probó y no pasó** (H4.3, PR #637).

## 2. Línea de tiempo

| Desde | Hasta | Qué pasa | En pantalla |
|---|---|---|---|
| [00:00](https://youtu.be/nlLGlVJGrtw?t=0) | [09:30](https://youtu.be/nlLGlVJGrtw?t=570) | Presentación y su filosofía: dejar de «predecir», buscar propiedades medibles y explotarlas. Ejemplo de la moneda con 65 % de cruz. | Solo cámaras |
| [09:30](https://youtu.be/nlLGlVJGrtw?t=570) | [23:00](https://youtu.be/nlLGlVJGrtw?t=1380) | Pizarra 1: validación (dentro y fuera de muestra, degradación, Sharpe, variantes). | Pizarra (§3.1) |
| [23:00](https://youtu.be/nlLGlVJGrtw?t=1380) | [30:30](https://youtu.be/nlLGlVJGrtw?t=1830) | Pizarra 1: tasa de aciertos contra expectativa; regímenes por percentil del ATR. | Pizarra (§3.2, §3.3) |
| [30:30](https://youtu.be/nlLGlVJGrtw?t=1830) | [46:00](https://youtu.be/nlLGlVJGrtw?t=2760) | Pizarra 1: autocorrelación de resultados y cómo la calcula. | Pizarra (§3.4) |
| [46:00](https://youtu.be/nlLGlVJGrtw?t=2760) | [60:00](https://youtu.be/nlLGlVJGrtw?t=3600) | Pizarra 2: frescura del régimen, eficiencia de precio (volumen), meta-label y Kelly. | Pizarra (§3.5–§3.7) |
| [60:00](https://youtu.be/nlLGlVJGrtw?t=3600) | [60:30](https://youtu.be/nlLGlVJGrtw?t=3630) | Publicidad de IQCapital (desafíos de 9 USD). | Placa de publicidad |
| [60:30](https://youtu.be/nlLGlVJGrtw?t=3630) | [79:30](https://youtu.be/nlLGlVJGrtw?t=4770) | Pizarra 3: la estrategia completa dibujada. | Pizarra (§3.8) |
| [79:30](https://youtu.be/nlLGlVJGrtw?t=4770) | [89:30](https://youtu.be/nlLGlVJGrtw?t=5370) | Ejemplo en TradingView: NQ, 15 minutos, repetición del viernes 18-09-2026. | Gráfico (§4) |
| [89:30](https://youtu.be/nlLGlVJGrtw?t=5370) | [98:30](https://youtu.be/nlLGlVJGrtw?t=5910) | Su software de estadísticas: métricas, simulación de prop firms, autocorrelación y Monte Carlo. | Software (§5) |
| [98:30](https://youtu.be/nlLGlVJGrtw?t=5910) | [103:09](https://youtu.be/nlLGlVJGrtw?t=6189) | Cierre: «no confíen en nadie, prueben»; publicidad de IQCapital. | Cámaras |

## 3. Las pizarras, una por una

### 3.1 Validación ([11:36](https://youtu.be/nlLGlVJGrtw?t=696)–[22:30](https://youtu.be/nlLGlVJGrtw?t=1350))

**Escrito [muestra]:** tres títulos arriba, «1 BACKTEST AND OVERFITTING», «2 WR» y «3 RISK MODEL / AUTOCORRELATION». Debajo, una curva de capital que sube con «2024–2025 / BUILD THE STRATEGY» y, a la derecha, «OUT SAMPLE 2026» con «EXPECTANCY / SHARPE RATIO». Arriba, «50 % 50 %» sobre dos curvas encerradas en círculos: una suave y una en serrucho. Debajo de la suave, «1,5>». Un rectángulo dividido en dos y, abajo, «0,2» y «50 %».

**Qué significa:**
- [dice] Las reglas se arman con 2024–2025 y se prueban sin tocarlas en 2026 ([14:00](https://youtu.be/nlLGlVJGrtw?t=840)). Si la expectativa y el Sharpe caen un 20–25 % es normal. Con 30 % es una alarma ([22:00](https://youtu.be/nlLGlVJGrtw?t=1320)). Con más del 50 % la estrategia «no es sólida». El ejemplo escrito: una expectativa de 0,2 que cae un 50 % ([21:00](https://youtu.be/nlLGlVJGrtw?t=1260)).
- [dice] Las dos curvas ganan lo mismo (50 %), pero la suave tiene mejor **Sharpe** (ganancia contra sus vaivenes). Un Sharpe por encima de 1,5 es bueno; menos de 1, no ([17:30](https://youtu.be/nlLGlVJGrtw?t=1050)).
- [dice] Cuantas más variantes probás, más fácil es encontrar una buena por casualidad. El rectángulo es su «optimización por bloques»: partir el backtest en unas 10 partes y ver si las métricas se mantienen ([18:30](https://youtu.be/nlLGlVJGrtw?t=1110)).
- [dice] Preguntado por qué el corte es 50 %, responde «porque lo probé» ([22:00](https://youtu.be/nlLGlVJGrtw?t=1320)). **[inferido]** No es un estándar: es su umbral personal, sin prueba mostrada.

### 3.2 Tasa de aciertos contra expectativa ([23:00](https://youtu.be/nlLGlVJGrtw?t=1380)–[24:30](https://youtu.be/nlLGlVJGrtw?t=1470))

**Escrito [muestra]:** tacha «WR» y escribe «EXPECTANCY». A la izquierda, «60 % 1:1» y, en un círculo, «COMMISSION / SLIPPAGE / SWAP / SPREAD». A la derecha, «0,2», «1,20 $» y «1000 → 1200 $».

**Qué significa:**
- [dice] La tasa de aciertos sola «es una estafa»: lo que importa es cuánto gana en promedio por cada dólar arriesgado.
- [inferido] El «60 % con 1:1» al lado de los costos es el ejemplo de una tasa alta que, después de comisión, slippage, financiación y spread, puede no dejar nada.
- [muestra] «0,2 → 1,20 $ → 1000 → 1200 $»: con expectativa 0,2, por cada 1.000 arriesgados vuelven 1.200 en promedio. **[inferido]** Es correcto si se lee como «la ganancia neta media es 200 por cada 1.000 arriesgados». Dicho como «el retorno promedio es 1.200» se presta a confusión.

### 3.3 Regímenes por volatilidad ([24:50](https://youtu.be/nlLGlVJGrtw?t=1490)–[30:00](https://youtu.be/nlLGlVJGrtw?t=1800))

**Escrito [muestra]:** «2,5 REGIMES» (lo numera 2,5 porque lo intercala) y, abajo, «LOW −0,10 · MEDIUM 0,3 · HIGH 0,6 (subrayado) · EXTREME −0,4». Encima, «ATR PERCENTILE: 0–25 LOW · 26–50 MEDIUM · 51–75 HIGH · 76–100 EXTREME». A la izquierda, «ATR LENGTH 14 / PERCENTILE LENGTH 200».

**Qué significa:**
- [dice] Mide la expectativa **por separado en cada régimen**. Los números de abajo son un ejemplo de expectativa en R por régimen: negativa en bajo y extremo, mejor en alto. Su estrategia rinde mejor con volatilidad **alta**.
- [muestra] Los parámetros quedan **escritos**: ATR de 14 velas y percentil sobre 200. Esto despeja la duda de §3.C del informe anterior («14 to 100»): es **200**.
- [inferido] El percentil del ATR es reproducible: dónde cae el ATR de hoy dentro de los últimos 200 valores.

### 3.4 Autocorrelación de resultados ([30:30](https://youtu.be/nlLGlVJGrtw?t=1830)–[46:00](https://youtu.be/nlLGlVJGrtw?t=2760))

**Escrito [muestra]:** «0 AUTOCORRELATION / POSITIVE / NEGATIVE» y la escala «0–2 · 3–5 · 6–8 · 9–10». Las secuencias «W → W → W → L → L → L» y «L–L–L–W–(W)», con la segunda W en un círculo. Un dibujo de precio en escalera que sube y después se mueve de costado en serrucho, con «W → W – W – L – L – L» debajo. A la izquierda, «40 %», «COWR», «W–L–L–W–W–L» y «2/6». A la derecha, «8 50 % → 4», «0–100» y «5».

**Qué significa:**
- [dice] Con autocorrelación positiva, las ganadoras y las perdedoras vienen en grupos. Su escala: 0–2 nula, 3–5 básica, 6–8 fuerte, 9–10 «probablemente sobreajuste» ([32:00](https://youtu.be/nlLGlVJGrtw?t=1920)).
- [muestra][dice] El dibujo de precio explica el **porqué**: una estrategia de tendencia gana seguido mientras el mercado sube en escalera y pierde seguido cuando el mercado pasa a moverse de costado ([36:30](https://youtu.be/nlLGlVJGrtw?t=2190)–[38:00](https://youtu.be/nlLGlVJGrtw?t=2280)).
- [muestra] La operación encerrada en «L–L–L–W–(W)» es la que él considera **la mejor**: la que viene después de la primera ganadora que corta una racha de pérdidas. Dice que la que sigue a una pérdida **no la toma** ([38:30](https://youtu.be/nlLGlVJGrtw?t=2310)).
- [muestra] **Cómo la calcula** ([43:30](https://youtu.be/nlLGlVJGrtw?t=2610)–[45:00](https://youtu.be/nlLGlVJGrtw?t=2700)): la tasa de aciertos normal (40 % en el ejemplo) contra la **condicional** («COWR»: solo las operaciones que vienen después de una pérdida). En la secuencia de ejemplo cuenta 2 de 6. Compara los dos porcentajes y «sale un número».
- [muestra] «8 50 % → 4»: si la autocorrelación cae un 50 % (de 8 a 4), sigue habiendo, pero más débil, y arriesga menos. «0–100» y «5»: se revalida cada 100 operaciones; si vuelve a 5, «hay autocorrelación otra vez» ([45:00](https://youtu.be/nlLGlVJGrtw?t=2700)–[45:30](https://youtu.be/nlLGlVJGrtw?t=2730)).
- **[inferido] Lo que falta:** nunca dice **cómo se pasa de dos porcentajes a la escala de 0 a 10**. Es una escala propia (en el software se llama «memory strength», §5), no una medida estándar.

### 3.5 Frescura del régimen ([46:00](https://youtu.be/nlLGlVJGrtw?t=2760)–[50:00](https://youtu.be/nlLGlVJGrtw?t=3000))

**Escrito [muestra]:** «4 REGIME FRESHNESS», «5 PRICE EFFICIENCY». Debajo, «BOCPD / BAYESIAN ONLINE CHANGE POINT DETECTION / REGIMES AGE 0–100» y «FRESH – MATURE – OLD: 0–30 · 31–60 · 61–100», con una tilde bajo «fresh».

**Qué significa:** [dice] un indicador gratuito de TradingView detecta cuándo cambió el régimen de volatilidad y da la «edad» del régimen. Su estrategia rinde mejor con un régimen **fresco** (0–30). [muestra en el gráfico, §4] El panel del indicador mide la edad **en velas** («Regime Age 60 bars»), no en una escala de 0 a 100. [inferido] Los cortes 0–30/31–60/61–100 parecen ser velas.

### 3.6 Eficiencia de precio: el volumen del disparador ([50:30](https://youtu.be/nlLGlVJGrtw?t=3030)–[55:30](https://youtu.be/nlLGlVJGrtw?t=3330))

**Dibujado [muestra]:** en un círculo, dos velas rojas que bajan y una azul que sube y cierra por encima de la mecha de arriba de la anterior. Debajo, las barras de volumen con una línea curva marcada «20». La vela azul tiene volumen **por debajo** de la línea. Después agrega otra vela azul que vuelve a superar a la anterior con **menos** volumen que ella, aunque todavía por encima de la línea. A la derecha escribe «REDUCING VOLUME ✓» y «LOW EFFORT».

**Qué significa:**
- [dice] No le importa el tamaño de la vela: le importa cuánto volumen («esfuerzo») hizo falta para el resultado. Hay **dos formas válidas**: «low effort» (volumen por debajo de la media de 20) y «reducing volume» (volumen menor que el de la vela anterior). La tilde marca a «reducing volume» como la mejor, la que suma un punto en el meta-label ([53:00](https://youtu.be/nlLGlVJGrtw?t=3180)).
- [dice] Llama a la media «EMA» y escribe solo «20» ([51:30](https://youtu.be/nlLGlVJGrtw?t=3090)). Sigue sin quedar claro si es exponencial o simple.
- [dice] Entrada al cierre de la vela, stop en su mínimo, objetivo 1:2. Probó de 1:1 a 1:5 y 1:2 fue el mejor ([55:00](https://youtu.be/nlLGlVJGrtw?t=3300)).

### 3.7 Meta-label y Kelly ([56:00](https://youtu.be/nlLGlVJGrtw?t=3360)–[59:40](https://youtu.be/nlLGlVJGrtw?t=3580))

**Escrito [muestra]:** «META LABEL» y cinco preguntas, cada una con una tilde:
1. «Is this setup in my best regime» (¿es mi mejor régimen?);
2. «Is this the best trade based on autocorrelation» (¿es la mejor operación según la racha?), con «L–L–W–(W)» al costado;
3. «Is the best freshness» (¿es la mejor frescura?);
4. «Which price efficiency» (¿qué tipo de volumen?);
5. «In which session we took it» (¿en qué sesión?).

Debajo, la tabla: «5/5 → ½ Kelly · 4/5 → ¼ Kelly · 3/5 → ⅕ Kelly · 2/5 → ⅙ Kelly».

**Qué significa:** [dice] cada sí suma un punto, y el puntaje decide qué fracción de Kelly arriesga. El Kelly completo «es absurdo». Con 1 de 5 **también opera**, pero arriesgando muy poco ([59:30](https://youtu.be/nlLGlVJGrtw?t=3570)). La sesión de Nueva York suma porque ahí rinde más que en Londres.
**[inferido]** La tabla escrita coincide con lo que §3.D del informe anterior sacó del audio. Ninguna de las cinco preguntas filtra: solo cambian el tamaño. **No lo adoptamos:** el proyecto arriesga 1 % fijo (§7 del informe anterior).

### 3.8 La estrategia completa ([60:30](https://youtu.be/nlLGlVJGrtw?t=3630)–[79:30](https://youtu.be/nlLGlVJGrtw?t=4770))

**Dibujado [muestra], en orden:**
1. A la izquierda, el **perfil de volumen de la sesión anterior** (un histograma acostado), con su área de valor marcada por líneas horizontales. Más tarde lo encierra en rojo ([61:00](https://youtu.be/nlLGlVJGrtw?t=3660)).
2. El precio sube en zigzag y, más arriba, aparece el **perfil de la sesión actual**, con su área de valor y su POC por encima de los anteriores: eso es la tendencia alcista ([60:30](https://youtu.be/nlLGlVJGrtw?t=3630)–[61:30](https://youtu.be/nlLGlVJGrtw?t=3690)).
3. El precio cae hasta el área de valor anterior. Arriba a la derecha dibuja un zoom: velas rojas que bajan y velas azules que suben y cruzan la línea del VAH, con volumen bajo debajo ([62:30](https://youtu.be/nlLGlVJGrtw?t=3750)–[63:30](https://youtu.be/nlLGlVJGrtw?t=3810)).
4. En el dibujo grande, las velas azules suben desde el área en escalera hasta el **máximo de la estructura** (una línea horizontal arriba), con la media del volumen («20») al costado: son la primera operación y las reentradas ([68:00](https://youtu.be/nlLGlVJGrtw?t=4080)–[69:30](https://youtu.be/nlLGlVJGrtw?t=4170)).
5. Arriba escribe «15 TF»: analiza y entra en 15 minutos, sin cambiar de marco ([70:00](https://youtu.be/nlLGlVJGrtw?t=4200)).
6. **Vela válida**, dos velas de ejemplo con números: una con cuerpo 4 y mecha 3 (válida) y otra con cuerpo 3 y mecha 6 (no válida) ([72:30](https://youtu.be/nlLGlVJGrtw?t=4350)–[74:20](https://youtu.be/nlLGlVJGrtw?t=4460)). [dice] Se mide en **ticks** con la regla de TradingView ([73:00](https://youtu.be/nlLGlVJGrtw?t=4380)).
7. Abajo, un dibujo chico marcado «15»: una subida, un retroceso y una ruptura. Marca en rojo el mínimo de la vela bajista válida más cercana a la ruptura: **ese es el mínimo de la estructura** ([73:30](https://youtu.be/nlLGlVJGrtw?t=4410)–[76:00](https://youtu.be/nlLGlVJGrtw?t=4560)).
8. Arriba a la derecha, al lado del zoom, escribe «0,6» y debajo «0,4»: el margen con el que el cierre tiene que superar el VAH. Con 0,6 vale y con 0,4 no ([77:30](https://youtu.be/nlLGlVJGrtw?t=4650)–[79:00](https://youtu.be/nlLGlVJGrtw?t=4740)).

**Lo que dice mientras dibuja:**
- [dice] Si el precio perfora el mínimo de la estructura antes del disparador, todo queda anulado: «ese disparador tiene expectativa negativa» ([71:30](https://youtu.be/nlLGlVJGrtw?t=4290)).
- [dice] Perforar el borde de abajo del área de valor **no** anula nada. Lo que anula es perforar el mínimo de la estructura ([64:30](https://youtu.be/nlLGlVJGrtw?t=3870)–[65:30](https://youtu.be/nlLGlVJGrtw?t=3930)).
- [dice] La primera operación es **siempre** la que rompe el VAH. Sin ella no hay reentradas ([69:30](https://youtu.be/nlLGlVJGrtw?t=4170)).
- [dice] Las posiciones abiertas se cierran todas en el máximo de la estructura. Si desde una entrada no hay lugar para 2 R hasta el máximo, puede entrar con menos, pero el objetivo nunca pasa del máximo ([71:00](https://youtu.be/nlLGlVJGrtw?t=4260)).
- [dice] No opera si hay un dato fuerte (NFP, CPI) **durante el disparador** ([77:00](https://youtu.be/nlLGlVJGrtw?t=4620)).
- [dice] Fuera de esto, el setup es «siempre operable» y tiene expectativa positiva en «los últimos tres años de datos» ([76:30](https://youtu.be/nlLGlVJGrtw?t=4590)). **[inferido]** En el software muestra dos años (§5).

## 4. El ejemplo en el gráfico ([79:30](https://youtu.be/nlLGlVJGrtw?t=4770)–[89:30](https://youtu.be/nlLGlVJGrtw?t=5370))

**Qué se ve [muestra]:** TradingView en modo repetición. Contrato continuo del NQ (`NQ1!`, CME), velas de 15 minutos, horario extendido («ETH») y serie ajustada («B-ADJ»). Indicadores: «SVP HD» (perfil de volumen por sesión) y volumen con una línea de media. Arriba a la derecha, el panel del indicador de cambio de régimen con estos campos: «Changepoint 2 %», «Regime Age 60 bars», «MAP Length 90 bars», «Hazard 2 %», «Monitor: Volatility» y «Mean Regime 50 bars». Día: **viernes 18-09-2026**. En la otra pestaña del navegador está abierto el software de estadísticas que usa («Caliber — Statistical Terminal»).

**Paso a paso:**
1. **Tendencia** ([80:00](https://youtu.be/nlLGlVJGrtw?t=4800)): el área de valor de la sesión en curso está por encima de la anterior. Lo clasifica como «mercado en tendencia alcista» [dice][muestra].
2. **Mínimo de la estructura** ([80:30](https://youtu.be/nlLGlVJGrtw?t=4830)–[82:00](https://youtu.be/nlLGlVJGrtw?t=4920)): mide la primera vela roja hacia atrás. Cuerpo 16 y mecha 56, así que no es válida. Las siguientes tampoco lo son «a ojo». Encuentra una con cuerpo 54 y mecha 24: válida. Su mínimo es el nivel que no se puede perforar [dice]. [inferido] Por la regla de TradingView, esos números son **ticks**.
3. **Régimen** ([83:00](https://youtu.be/nlLGlVJGrtw?t=4980)): «la edad del régimen es 60 velas, así que es maduro o viejo». [inferido] Con sus propios cortes, 60 cae en «maduro». Igual opera: el régimen no es filtro.
4. **Primera vuelta al área** ([83:30](https://youtu.be/nlLGlVJGrtw?t=5010)–[84:30](https://youtu.be/nlLGlVJGrtw?t=5070)): el precio entra al área anterior solo con una mecha, «y alcanza». Rompe y cierra arriba, pero el volumen **sube** y está por encima de la media: no hay entrada. «Si la hubiéramos tomado, habría sido stop» [dice].
5. **Segunda señal, prematura** ([84:30](https://youtu.be/nlLGlVJGrtw?t=5070)–[85:30](https://youtu.be/nlLGlVJGrtw?t=5130)): una vela supera a la anterior. Mide el margen y la regla marca **«6.00 (0.02 %) 24»**, es decir 6 puntos o 24 ticks [muestra]. Él lee «24» y dice «24 es más que 6» [dice]. El volumen baja, pero **todavía no rompió el VAH**, así que no entra. También habría sido stop [dice].
6. **La entrada** ([86:00](https://youtu.be/nlLGlVJGrtw?t=5160)–[87:00](https://youtu.be/nlLGlVJGrtw?t=5220)): una vela supera a la anterior por solo 3 («3 no es 6»: no vale). La siguiente rompe el VAH, supera a la anterior por más de 6 y tiene el volumen en subida pero **por debajo de la media**. Entra [dice][muestra].
7. **Objetivo** ([87:30](https://youtu.be/nlLGlVJGrtw?t=5250)): la herramienta de posición marca objetivo 56,00 puntos (224 ticks), stop de unos 28 puntos (113 ticks) y «Risk/reward ratio 1.98» [muestra]. Llega a 2 R.
8. **Reentrada** ([88:30](https://youtu.be/nlLGlVJGrtw?t=5310)–[89:00](https://youtu.be/nlLGlVJGrtw?t=5340)): otra vela supera a la anterior por 25 con el volumen por debajo de la media. Entra de nuevo y llega al objetivo en una vela. Después, el volumen sube («mucho esfuerzo») y el precio llega al máximo de la estructura: no hay más operaciones [dice].

**Qué aporta el gráfico [inferido]:**
- **El margen «6» del Nasdaq es en ticks.** La regla mostró 6 puntos = 24 ticks y él comparó el 24 con el 6. Entonces el mínimo es **6 ticks = 1,5 puntos** (el tick del NQ es 0,25). Así lo congeló H4.3 (m = 1,5 puntos). En forex, «0,6» queda sin unidad clara.
- **Sesión = día con horario extendido.** El gráfico está en «ETH» y el perfil es por sesión, lo que apunta a la sesión completa del CME (18:00–17:00 ET), como usó H4.3.
- Es **un solo día elegido** por él para explicar, y además viernes de vencimiento trimestral (se ve `NQZ2026` junto al precio). Muestra dos señales que habrían perdido y que él descarta por las reglas; eso es coherente, pero un ejemplo no prueba nada.

## 5. El software de estadísticas ([89:30](https://youtu.be/nlLGlVJGrtw?t=5370)–[98:30](https://youtu.be/nlLGlVJGrtw?t=5910))

**Qué se ve [muestra]:** una web llamada «Caliber, Statistical Terminal Analysis», con la sesión iniciada como «gianlucabruni.g» y estrategias en pestañas («Championship», «PX», «test», «Axiom»). La estrategia «Championship», todos los instrumentos, con el historial completo de **septiembre de 2024 a septiembre de 2026**, 727 operaciones y riesgo de 0,50 % por operación.

| Métrica (en pantalla) | Valor | Nota [inferido] |
|---|---|---|
| R neto / retorno | +147,04 R / +103,9 % | Dos años con 0,5 % fijo. No es el 104 % «en un trimestre» de la presentación: son cosas distintas. |
| Expectativa | +0,20 R | Cuadra: 42 % × 1,95 − 45,3 % × 1,35 ≈ 0,21 R (el 12,7 % restante son operaciones en cero). |
| Aciertos / punto de equilibrio | 42,0 % / 40,0 % | Margen chico sobre el equilibrio. |
| Ganadora media / perdedora media | +1,95 R / −1,35 R | **Ojo:** con stop en 1 R, una perdedora media de 1,35 R quiere decir que costos y slippage pesan mucho, o que hay pérdidas mayores que el stop. No lo explica. |
| Sharpe / Sortino / Calmar | 2,20 / 3,65 / 3,30 | |
| Máxima caída / tiempo bajo el máximo | −13,1 % / 84,3 % | |
| Racha perdedora máxima | 15 | |
| Estadístico t | 3,48 (727 operaciones) | |
| «Ahora mismo» | −13,1 %, 98 días bajo el máximo | **Al grabar, estaba en su peor caída.** |
| «Últimas 100 operaciones» | −0,03 R | **En la pantalla, las últimas 100 dan pérdida.** Él cita la media móvil de 60 (0,17 R) y no comenta esta. |
| Concentración | 6,7 % de la ganancia en las 5 mejores | |

**Autocorrelación ([95:30](https://youtu.be/nlLGlVJGrtw?t=5730)) [muestra]:** «memory strength 7,9» de 10 y «memory confidence 10,0». Después de una ganadora (306 casos), la siguiente gana el **52,5 %**; después de una perdedora (421 casos), el **34,2 %**. La base es 42,0 %, con banda de 95 % de 38,4–45,6 %. Tres pruebas: Ljung-Box Q 57,51 (valor crítico 18,31), Hurst 0,63 y razón de varianzas 1,20. [inferido] Los números cuadran entre sí: 306 + 421 = 727, y 160 + 144 ganadoras ≈ 42 % de 727. Pero son **dentro de muestra** y del mismo historial que se usó para armar la regla.

**Simulaciones ([91:00](https://youtu.be/nlLGlVJGrtw?t=5460)–[98:00](https://youtu.be/nlLGlVJGrtw?t=5880)):**
- [dice][muestra] 200 desafíos de prop firm: «pasamos el 40 %», en el mejor caso 92 de 200 (eso es 46 %) y en el peor 71. «Gastamos casi 10.000 USD para ganar 184.000 USD de drawdown adquirido». [inferido] «Drawdown adquirido» es el margen de pérdida de las cuentas fondeadas, **no dinero ganado**.
- [dice] 200 cuentas fondeadas: «600.000 USD» si se cumplen todas las reglas y se cobran todos los pagos. Él mismo aclara que supone ejecución perfecta.
- [dice] Monte Carlo con 10.000 órdenes al azar: 41 terminan perdiendo y el peor llega a −150 R.

**Alertas [inferido]:**
- [dice] Los datos **se cargan a mano** («tenés que subirlo manualmente») y el software **no es gratis** ([96:00](https://youtu.be/nlLGlVJGrtw?t=5760)). Es un backtest que no se puede auditar, mostrado en un software que él usa con su usuario. No sabemos si es suyo o si tiene relación comercial con él: hay que tomarlo como posible conflicto de interés, sumado a la prop firm que publica el video.
- Lo que muestra el software es el historial a **riesgo fijo**. La ganancia extra que atribuye al modelo de riesgo (Kelly fraccional, «la mitad de lo que gané») **no aparece en pantalla**.

## 6. Correcciones y precisiones al informe anterior

- **Percentil del ATR: 200, no 100.** Está escrito en la pizarra ([29:00](https://youtu.be/nlLGlVJGrtw?t=1740)). Despeja la duda de §3.C.
- **Margen del Nasdaq: 6 ticks (1,5 puntos).** La regla de TradingView del ejemplo lo confirma (§4). Pasa de [inferido] a [muestra].
- **Edad del régimen: en velas.** El panel dice «60 bars». Los cortes 0–30/31–60/61–100 parecen ser velas, no una escala de 0 a 100.
- **Tabla de Kelly**, escrita: 5/5 ½, 4/5 ¼, 3/5 ⅕, 2/5 ⅙. Coincide con §3.D.
- **«Expectativa positiva en los últimos tres años»:** en el software se ven dos (09-2024 a 09-2026). Y las últimas 100 operaciones dan −0,03 R.

## 7. Para la Academia

Una lección sobre este video, de lo más simple a lo más difícil. Encaja con el caso Bruni del nivel 10 (`docs/academia/nivel-10-7-caso-bruni.md`) y con el perfil de volumen del nivel 11 (`nivel-11-2-perfil-de-volumen.md`).

1. **Medir, no opinar.** Su frase final, «no confíen en nadie, prueben», es la idea de toda la Academia. Ejercicio: escribir en una línea una regla propia que otra persona pueda repetir sin preguntar.
2. **Expectativa antes que aciertos.** Con la pizarra de §3.2: 60 % de aciertos a 1:1 parece bueno hasta que se restan los costos. Calcular la expectativa en R de su propio software: 42 % de aciertos y aun así +0,20 R (nivel 8).
3. **Dentro y fuera de muestra.** Armar con un período y probar en otro sin tocar nada; qué degradación es normal; por qué probar muchas variantes engaña (nivel 9).
4. **La vela válida.** Medir cuerpo y mecha en ticks. Ejercicio de repetición: clasificar 20 velas como válidas o no.
5. **El perfil de volumen por sesión.** Área de valor, VAH, VAL y POC; qué quiere decir que el área de hoy esté más arriba que la de ayer.
6. **El mínimo de la estructura sin opiniones.** Buscar hacia atrás la vela bajista válida más cercana a la ruptura.
7. **El setup completo**, paso a paso con el ejemplo del 18-09-2026 en NQ: espera, primera señal descartada por el volumen, segunda descartada por no romper el VAH, entrada, 2 R, reentrada y cierre en el máximo.
8. **El volumen como «esfuerzo».** Las dos formas válidas y por qué el tamaño de la vela no importa en su regla.
9. **Regímenes.** Percentil del ATR de 14 sobre 200 y la expectativa por régimen (nivel 10).
10. **Autocorrelación de resultados.** La tasa de aciertos condicional contra la normal, con la secuencia de la pizarra (2 de 6 contra 40 %). Por qué con pocas operaciones el ruido lo tapa todo (1/√N) y por qué en nuestros datos de H4.1 y H4.2 salió al revés.
11. **El tamaño según el puntaje (meta-label y Kelly).** Se explica para entenderlo y se marca como **lo que no hacemos**: el proyecto arriesga 1 % fijo.
12. **Leer un backtest ajeno con ojo crítico.** Con la tabla de §5: dónde mirar (últimas 100 operaciones, caída actual, perdedora media mayor que 1 R, datos cargados a mano, quién vende qué). Cierre con el resultado de H4.3 en nuestros datos.

## 8. Para backtestear

**Antes de nada:** esta estrategia **ya se probó** en el CFD US100, 2013–2025, con regla congelada: **H4.3 no pasa** (85 operaciones, −0,31 R netos por operación y −0,17 R antes de costos; `docs/analysis/2026-11-programa-4-h43-resultados.md`, PR #637). Según el pre-registro, **no se prueban variantes**. Cualquier prueba nueva necesita un pre-registro nuevo y su lugar en el presupuesto global de error (CLAUDE.md §5b). Las definiciones que siguen sirven para eso, o para la Academia.

Todo se evalúa **al cierre** de la vela `t` de 15 minutos, con velas cerradas, y se ejecuta en `t+1`. Lo que sigue es para largos; los cortos serían el espejo [inferido: él solo muestra largos].

| # | Regla | Definición medible | De dónde | H4.3 usó |
|---|---|---|---|---|
| R1 | Vela válida | `abs(close − open) > max(high − max(open, close), min(open, close) − low)`, en ticks | [muestra] pizarra y gráfico | igual |
| R2 | Perfil por sesión | Perfil de la sesión completa del CME (18:00–17:00 ET); área de valor = 70 % del volumen alrededor del POC | [muestra] «SVP HD», «ETH»; [dice] 70 % | 18:00–17:00 ET, casillero 0,0002 |
| R3 | Tendencia alcista | `VAH_dev(t) > VAH_prev`, `VAL_dev(t) > VAL_prev` y `POC_dev(t) > POC_prev`, con el perfil en desarrollo hasta `t` | [muestra] dibujo y gráfico | ver pre-registro |
| R4 | Mínimo de la estructura | Desde la ruptura de la estructura, hacia atrás: el mínimo de la vela bajista válida (R1) más cercana | [muestra] pizarra y gráfico | igual |
| R5 | Máximo de la estructura | El máximo anterior (el techo desde el que vino el retroceso) | [dice] | tope del objetivo |
| R6 | Setup activo | Con R3 vigente, `low(t) ≤ VAH_prev` alguna vez (alcanza una mecha), sin que `low < mínimo de la estructura` | [muestra] gráfico | igual |
| R7 | Disparador (1.ª operación) | En la misma vela: `close(t) − high(t−1) ≥ m`, `close(t) − VAH_prev ≥ m` y volumen eficiente (R8), con `m` = 6 ticks en NQ | [muestra] regla «24» y «0,6/0,4» | m = 1,5 puntos |
| R8 | Volumen eficiente | `vol(t) < vol(t−1)` **o** `vol(t) < MA20(vol)` | [dice][muestra] «20» | SMA 20 |
| R9 | Orden | Entrada al cierre de `t` (apertura de `t+1` en el backtest), stop = `low(t)`, objetivo = entrada + 2 × riesgo, con tope en R5 | [dice]; [muestra] R:R 1,98 | igual |
| R10 | Reentradas | Después de la 1.ª operación, cada vela con `close(t) − high(t−1) ≥ m` y R8 abre otra, hasta que el precio toque R5 | [muestra] dibujo y gráfico | igual |
| R11 | Anulación | Si `low < mínimo de la estructura` antes del disparador, el setup se cancela | [dice] | igual |
| R12 | Noticias | No operar si hay un dato fuerte (NFP, CPI) durante la vela del disparador | [dice] | todo el día (FOMC, CPI, empleo) |
| R13 | Régimen (solo tamaño) | Percentil del ATR(14) sobre las últimas 200 velas: 0–25 / 26–50 / 51–75 / 76–100 | [muestra] pizarra | descriptivo |
| R14 | Frescura (solo tamaño) | Edad del régimen (BOCPD) en velas: 0–30 / 31–60 / 61–100 | [muestra] panel | no |
| R15 | Racha (solo tamaño) | «Mejor operación» = la siguiente a la primera ganadora tras ≥ 1 perdedora | [muestra] «L–L–W–(W)» | no |
| R16 | Costos | Spread, comisión, financiación y slippage, nunca en cero | [muestra] círculo de costos | los del CFD de IG |

**Preguntas abiertas (él es vago o no lo dice):**
1. **Ruptura de la estructura:** no define en este video qué cuenta como ruptura (cuántas velas confirman un máximo; si cierra el cuerpo o alcanza la mecha). H4.3 usó la definición común del programa 4 (§3 del pre-registro).
2. **Tendencia:** ¿alcanza con que el área se haya corrido hacia arriba, o tiene que estar **toda** por encima de la anterior? ¿Desde qué hora de la sesión vale el perfil en desarrollo?
3. **Margen en forex:** «0,6» ¿en pips, en décimas de pip o en el número de ticks de la regla?
4. **Media del volumen:** ¿exponencial o simple? ¿Incluye la vela `t`?
5. **Empates en R1** (cuerpo igual a la mecha): no lo dice.
6. **Reentradas:** ¿puede abrir una mientras la anterior sigue abierta? ¿Hay un máximo? En el ejemplo, la segunda llega después de que la primera tocó el objetivo.
7. **Noticias:** ¿solo la vela del dato, o «los días con muchas noticias»? Dice las dos cosas.
8. **Sesión de Nueva York** (un punto del meta-label): no da el horario.
9. **Escala de autocorrelación 0–10:** no explica cómo se pasa de las tasas condicionales a ese número.
10. **Frescura:** el indicador de TradingView no tiene una definición pública que se pueda reproducir igual. Se ven sus parámetros (cambio 2 %, «hazard» 2 %, largo 90 velas), pero no el código.
11. **Perdedora media de −1,35 R:** ¿de dónde sale, si el stop es 1 R? Hace falta para modelar bien los costos.

## 9. Fuentes

- Video: IQCapital, 02-10-2026, https://www.youtube.com/watch?v=nlLGlVJGrtw. Fotogramas y subtítulos automáticos en inglés analizados el 26-11-2026. El video y los fotogramas quedan **fuera del repo**, solo en el Mac mini del dueño.
- Informe previo: `docs/research/2026-11-gianluca-bruni.md` (§3.C y §3.D, por el audio con Whisper).
- Prueba en nuestros datos: `docs/analysis/2026-11-programa-4-preregistro.md` y `docs/analysis/2026-11-programa-4-h43-resultados.md` (#561, PR #637).
- El otro video de Bruni, estudiado igual: #853.
