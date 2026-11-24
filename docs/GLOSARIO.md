# Glosario

Cada término tiene una explicación corta y un ejemplo. Los otros documentos enlazan acá: hacé clic en una palabra subrayada para saltar a su definición.
Los ejemplos usan el US Tech 100 (NASDAQ) cerca de 26.750 puntos, como en el gráfico que compartimos.

> Más adelante la UI (fase 10) va a mostrar estas mismas definiciones al pasar el mouse o tocar cada término. Este archivo es la **única fuente** de las definiciones: si cambia una, cambia acá.

**Índice:** [Operar](#operar) · [Costos](#costos) · [Gestión de la operación](#gestion) · [Velas e indicadores](#velas) · [Patrones](#patrones) · [Liquidez (ICT/SMC)](#liquidez-ict) · [Medir resultados](#medir) · [Probar sin engañarse](#probar) · [Psicología del mercado](#psicologia) · [Fondo del grupo](#fondo) · [Empresas (no aplica a índices)](#empresas)

---

<a id="operar"></a>
## Operar

<a id="cfd"></a>
### CFD (Contrato por Diferencia)
Un contrato con el broker por el que ganás o perdés la **diferencia de precio** de un activo, sin comprarlo de verdad. Se puede apostar a la suba (**largo**) o a la baja (**corto**).
*Ejemplo:* abrís un largo de 1 CFD del NASDAQ en 26.750 y cerrás en 26.850: ganás 100 puntos × el valor por punto del contrato.

<a id="largo-corto"></a>
### Largo / Corto
**Largo** = ganás si sube. **Corto** = ganás si baja.
*Ejemplo:* si esperás que el NASDAQ caiga tras una mala noticia, abrís un corto.

<a id="apalancamiento"></a>
### Apalancamiento
Operar con más dinero del que ponés. Con 10:1, cada 1 € de garantía mueve 10 € de posición. **Multiplica las ganancias y las pérdidas.**
*Ejemplo:* con 1.000 € y apalancamiento 10:1 controlás una posición de 10.000 €. Si el NASDAQ cae 2%, perdés 200 €, que es el 20% de tus 1.000 €.

<a id="margen"></a>
### Margen
La garantía que el broker te bloquea para mantener abierta una posición apalancada. Margen = tamaño de la posición ÷ apalancamiento.
*Ejemplo:* una posición de 10.000 € con apalancamiento 20:1 requiere 500 € de margen.

<a id="stop-out"></a>
### Margin call / Stop-out
Si tus pérdidas se comen el margen, el broker primero avisa (**margin call**) y después **cierra tus posiciones a la fuerza** (stop-out) cuando el margen cae por debajo de un nivel (por ejemplo 50%).
*Ejemplo:* tenés 500 € de margen y la posición pierde 260 €. Te queda menos del 50% y el broker la cierra sola.

<a id="esma"></a>
### Regulador / ESMA
Organismos que fijan límites para proteger al cliente minorista. En la UE (ESMA) el apalancamiento máximo es 30:1 en forex principal, 20:1 en índices principales y 5:1 en acciones.

<a id="timeframe"></a>
### Timeframe
Cuánto tiempo representa cada vela del gráfico.
*Ejemplo:* en 15m cada vela son 15 minutos. Usamos **1h para ver la tendencia** y **15m para entrar**.

<a id="paper-trading"></a>
### Paper trading (simulación)
Operar con dinero ficticio sobre precios reales, para probar sin arriesgar.

<a id="halt"></a>
### Halt
Suspensión temporal de la cotización de una acción o de un mercado, por noticias o por caídas bruscas.

<a id="gap"></a>
### Gap
Salto de precio sin operaciones en el medio. Típico entre el cierre del viernes y la apertura del lunes.
*Ejemplo:* el viernes cierra en 26.750 y el lunes abre en 26.400. Un stop en 26.600 se ejecuta en 26.400: perdés más de lo planeado.

---

<a id="costos"></a>
## Costos

<a id="spread"></a>
### Spread
Diferencia entre el precio de compra y el de venta. Es un costo que pagás al entrar.
*Ejemplo:* en tu captura, SELL 26.747,8 y BUY 26.748,8: el spread es 1 punto.

<a id="swap"></a>
### Swap / Financiación overnight
Interés que cobra (o, a veces, paga) el broker por mantener un CFD abierto de un día para otro.
*Ejemplo:* dejás un largo abierto 3 noches y pagás 3 cargos de financiación, aunque el precio no se haya movido.

<a id="slippage"></a>
### Slippage (deslizamiento)
Diferencia entre el precio que querías y el que realmente obtuviste, porque el mercado se movió.
*Ejemplo:* mandás comprar en 26.750 y te ejecutan en 26.753: 3 puntos de slippage.

<a id="comision"></a>
### Comisión
Lo que cobra el broker por operación. En CFDs de índices suele estar incluida en el spread; en CFDs de acciones a veces se cobra aparte.

---

<a id="gestion"></a>
## Gestión de la operación

<a id="stop-loss"></a>
### Stop loss (SL)
Orden que cierra la operación automáticamente si el precio va en tu contra hasta cierto nivel. Limita la pérdida.
*Ejemplo:* largo en 26.750 con SL en 26.700: como máximo perdés 50 puntos (salvo un [gap](#gap)).

<a id="take-profit"></a>
### Take profit (TP)
Orden que cierra la operación automáticamente al llegar a la ganancia objetivo.
*Ejemplo:* largo en 26.750 con TP en 26.850.

<a id="r-multiplo"></a>
### R (múltiplo de riesgo)
1R = lo que arriesgás en una operación (la distancia al stop). Un TP en "2R" busca ganar el doble de lo que arriesgás.
*Ejemplo:* stop a 50 puntos → 1R = 50, así que un TP en 2R está a 100 puntos.

<a id="trailing-stop"></a>
### Trailing stop
Stop que se va moviendo a favor a medida que el precio avanza, para asegurar ganancia.
*Ejemplo:* stop a 50 puntos del máximo alcanzado. Si el precio sube a 26.900, el stop pasa a 26.850.

<a id="riesgo-por-operacion"></a>
### Riesgo por operación
El % del capital que estás dispuesto a perder si salta el stop. De ahí sale el tamaño de la posición.
*Ejemplo:* capital de 10.000 € y riesgo del 1% = 100 €. Con stop a 50 puntos, la posición es de 2 € por punto.

<a id="kill-switch"></a>
### Kill switch
Freno de emergencia: el sistema deja de operar (y puede cerrar todo) si se supera un límite de pérdida, si los datos fallan o si se cae la conexión.

---

<a id="velas"></a>
## Velas e indicadores

<a id="vela"></a>
### Vela japonesa (OHLC)
Cada vela muestra 4 precios del período: apertura (Open), máximo (High), mínimo (Low) y cierre (Close). Verde si cerró más arriba de donde abrió, roja si cerró más abajo. El "cuerpo" va de la apertura al cierre y las "mechas" llegan al máximo y al mínimo.
*Ejemplo:* en tu captura, O 26.756,6 · H 26.760,8 · L 26.732,3.

<a id="vela-cerrada"></a>
### Vela cerrada
Una vela cuyo período ya terminó. Sus valores ya no cambian. **Solo decidimos con velas cerradas.**

<a id="martillo"></a>
### Martillo (Hammer)
Vela de cuerpo chico arriba y mecha inferior larga (al menos el doble del cuerpo), después de una caída. Indica que los vendedores empujaron hacia abajo pero los compradores recuperaron: posible giro alcista.
*Ejemplo:* en tu captura aparece marcada como "Hammer – Bull SMA50".

<a id="estrella-fugaz"></a>
### Estrella fugaz (Shooting star)
El espejo del [martillo](#martillo): cuerpo chico abajo y mecha superior larga, después de una suba. Los compradores empujaron arriba pero los vendedores devolvieron el precio: posible giro bajista.

<a id="envolvente"></a>
### Envolvente (Engulfing)
Vela cuyo cuerpo "envuelve" por completo el cuerpo de la vela anterior, en dirección contraria. La **envolvente alcista** (verde que envuelve a una roja) sugiere giro al alza.

<a id="sma"></a>
### SMA (media móvil simple) / SMA50
Promedio de los últimos N cierres. La SMA50 promedia 50 velas. Si el precio está por encima, la tendencia se considera alcista.
*Ejemplo:* "Bull SMA50" = la señal solo vale si el precio está por encima de la SMA50.

<a id="ema"></a>
### EMA (media móvil exponencial)
Como la SMA, pero da más peso a las velas recientes, así que reacciona más rápido.
El período se elige en la configuración. En el gráfico mostramos además una **EMA 365 sobre velas diarias** (`config/analysis.toml`): cada vela de 15m ve el valor del último día ya terminado, nunca el del día en curso. 365 velas diarias son unos **1,45 años** de sesiones (un año tiene unas 252); la media "larga" más usada en el mercado es la de **200** días. Es solo para mirar: no entra en la estrategia.

<a id="rsi"></a>
### RSI
Oscilador de 0 a 100 que mide la fuerza de las subas frente a las bajas. Por encima de 70 se habla de "sobrecompra" y por debajo de 30 de "sobreventa".

<a id="atr"></a>
### ATR
Rango promedio que se mueve el precio por vela. Mide la volatilidad y se usa para poner stops a una distancia razonable.
*Ejemplo:* si el ATR de 15m es 40 puntos, un stop de 10 puntos salta por puro ruido.

<a id="vwap"></a>
### VWAP
Precio promedio del día ponderado por el volumen. Muchos institucionales lo usan como referencia de "precio justo" intradía.

<a id="volumen"></a>
### Volumen
Cuánto se operó en esa vela. Una ruptura con mucho volumen es más confiable.

<a id="tendencia"></a>
### Tendencia
Dirección general del precio. **Alcista** = máximos y mínimos cada vez más altos. **Bajista** = cada vez más bajos.

---

<a id="patrones"></a>
## Patrones

<a id="pivote"></a>
### Pivote (máximo o mínimo local)
Una vela cuyo máximo (o mínimo) es el más extremo de las N velas a cada lado. **Solo se confirma N velas después**: antes no podemos saber si era un pivote.
*Ejemplo:* con N = 3, un máximo de las 10:00 se confirma recién a las 10:45 en 15m.

<a id="soporte"></a>
### Soporte
Nivel de precio donde el precio rebotó hacia arriba varias veces (compradores esperando ahí).
*Ejemplo:* el NASDAQ tocó 26.600 tres veces y rebotó cada vez.

<a id="resistencia"></a>
### Resistencia
Lo opuesto: nivel donde el precio fue rechazado hacia abajo varias veces.

<a id="ruptura"></a>
### Ruptura (breakout)
Cuando una vela **cierra** del otro lado de un soporte o una resistencia.

<a id="retesteo"></a>
### Retesteo
Después de una ruptura, el precio vuelve a tocar el nivel roto antes de seguir. Una resistencia rota suele convertirse en soporte.

<a id="doble-techo"></a>
### Doble techo / Doble suelo
**Doble techo:** dos máximos casi iguales separados por una caída. Señal bajista cuando el precio rompe el [neckline](#neckline). El **doble suelo** es lo mismo, al revés.
*Ejemplo:* máximos en 26.900 y 26.890 con un valle en 26.700 entre ellos. Al cerrar una vela de 15m por debajo de 26.700, se confirma.

<a id="neckline"></a>
### Neckline (línea de cuello)
El valle entre los dos techos (o el pico entre los dos suelos). Su ruptura confirma el patrón. **Nosotros entramos al cerrar la vela que rompe el neckline.**

<a id="numero-de-oro"></a>
### Número de oro (φ)
El número φ = (1 + √5) / 2 ≈ **1,618**. Aparece en la naturaleza y en el arte, y es la base de los niveles de [Fibonacci](#fibonacci): 0,618 ≈ 1/φ, 0,382 ≈ 1/φ², 0,236 ≈ 1/φ³, 1,618 = φ y 2,618 ≈ φ². No es un indicador aparte: es de donde salen esos números. El motor lo guarda como decimal exacto (28 decimales), nunca como número aproximado.

<a id="fibonacci"></a>
### Fibonacci (retrocesos y extensiones)
Niveles que se dibujan sobre un **tramo**: el movimiento entre dos [pivotes](#pivote) confirmados seguidos, uno bajo y uno alto. Los **retrocesos** (23,6 %, 38,2 %, 50 %, 61,8 % y 78,6 %) marcan hasta dónde podría volver el precio antes de seguir; las **extensiones** (127,2 %, 161,8 % y 261,8 %) marcan hasta dónde podría llegar si sigue más allá del tramo. Muchos traders los miran, y por eso a veces el precio reacciona ahí, pero **no garantizan nada**: son zonas para observar, no predicciones.
*Ejemplo:* el NASDAQ sube de 26.600 a 26.800 (tramo de 200 puntos). El retroceso del 61,8 % está en 26.800 − 0,618 × 200 = **26.676,4**; la extensión de 161,8 % está en 26.600 + 1,618 × 200 = **26.923,6**.
El tramo existe recién cuando se confirma su pivote final (N velas después). Si una vela cierra más allá del inicio del tramo, ese tramo deja de valer.

<a id="golden-pocket"></a>
### Golden pocket (bolsillo dorado)
La zona entre los retrocesos de **61,8 % y 65 %** de un tramo de [Fibonacci](#fibonacci). Es la zona de retroceso más vigilada; marcamos las velas que la **tocan** y las que **cierran adentro**.
*Ejemplo:* en el tramo de 26.600 a 26.800, el golden pocket va de 26.670 a 26.676,4. Una vela que baja hasta 26.672 la toca. Es contexto para mirar, no una señal de compra.

---

<a id="liquidez-ict"></a>
## Liquidez (marco ICT/SMC)

<a id="liquidez"></a>
### Liquidez
Zonas donde se acumulan órdenes pendientes, sobre todo stops de otros traders, típicamente justo arriba de máximos y abajo de mínimos visibles. Los grandes participantes "van a buscar" esa liquidez para llenar sus órdenes.

<a id="liquidez-externa"></a>
### Liquidez externa
La que está **afuera del rango** actual: arriba del máximo y abajo del mínimo del swing mayor.
*Ejemplo:* el rango de la semana va de 26.400 a 26.900. Arriba de 26.900 hay stops de los que están en corto: liquidez externa.

<a id="liquidez-interna"></a>
### Liquidez interna
La que está **dentro del rango**: [fair value gaps](#fvg) y swings menores. El precio suele ir de liquidez interna a externa y viceversa.

<a id="sweep"></a>
### Sweep (barrido de liquidez)
El precio pasa brevemente un máximo o mínimo (activando los stops) y **vuelve rápido al rango**. Suele anticipar un giro.
*Ejemplo:* el precio pincha 26.905, por arriba del máximo de 26.900, y la vela cierra en 26.860: barrió la liquidez de arriba.

<a id="order-block"></a>
### Order block
La última vela contraria antes de un movimiento fuerte. Se interpreta como la zona donde entraron órdenes grandes y suele funcionar como soporte o resistencia al volver.

<a id="fvg"></a>
### Fair value gap (FVG)
Hueco entre la mecha de la vela 1 y la mecha de la vela 3 de un movimiento fuerte de 3 velas. Zona "desequilibrada" que el precio tiende a volver a llenar.

<a id="bos"></a>
### BOS (Break of Structure)
Ruptura de un máximo (o mínimo) previo **a favor** de la tendencia: la tendencia continúa.

<a id="choch"></a>
### CHoCH (Change of Character)
Primera ruptura **en contra** de la tendencia: posible cambio de dirección.

---

<a id="medir"></a>
## Medir resultados

<a id="pl"></a>
### P/L (P&L, ganancias y pérdidas)
Resultado en dinero. **Realizado** = de operaciones cerradas. **No realizado** = de operaciones todavía abiertas.

<a id="drawdown"></a>
### Drawdown
Caída desde el punto más alto del capital hasta el más bajo siguiente.
*Ejemplo:* el capital sube a 12.000 € y baja a 10.800 €: drawdown del 10%.

<a id="win-rate"></a>
### Win rate
% de operaciones ganadoras. Por sí solo no dice nada: se puede ganar plata con un 40% de aciertos si las ganancias son el doble que las pérdidas.

<a id="profit-factor"></a>
### Profit factor
Ganancias totales ÷ pérdidas totales. Por encima de 1, el sistema gana dinero.

<a id="sharpe"></a>
### Sharpe / Sortino
Retorno ajustado por riesgo: cuánto ganás por cada unidad de volatilidad. El Sortino solo penaliza la volatilidad a la baja.

<a id="probabilidad"></a>
### Probabilidad de la señal
Estimación (no garantía) de que la operación llegue al TP antes que al SL, calculada con datos históricos de situaciones parecidas.

---

<a id="probar"></a>
## Probar sin engañarse

<a id="backtest"></a>
### Backtest
Simular la estrategia sobre datos históricos para ver cómo le habría ido.

<a id="look-ahead"></a>
### Look-ahead (mirar el futuro)
Error de usar en el pasado información que en ese momento todavía no existía. Hace que el backtest parezca mejor de lo real.
*Ejemplo:* decidir a las 10:00 usando el cierre de la vela de 10:00 a 10:15.

<a id="repintado"></a>
### Repintado
Cuando un indicador cambia sus valores pasados al llegar datos nuevos. En el gráfico se ve perfecto, pero en vivo no se puede replicar.

<a id="sobreajuste"></a>
### Sobreajuste (overfitting)
Ajustar tanto los parámetros al pasado que la estrategia "memoriza" el histórico y falla en el futuro.

<a id="fuera-de-muestra"></a>
### Datos fuera de muestra
Datos guardados aparte que nunca se usan para ajustar nada. Sirven para la prueba final honesta.

<a id="walk-forward"></a>
### Walk-forward
Ajustar con un período, probar en el siguiente, avanzar y repetir. Simula cómo se usaría el sistema en la realidad.

<a id="analogos"></a>
### Análogos históricos
Situaciones del pasado (en este u otros activos) parecidas a la actual, para ver qué pasó después.
*Ejemplo:* "las últimas 40 veces que el NASDAQ hizo un doble techo en 1h con RSI > 70, cayó al menos 0,5% en 24 de ellas".

<a id="sentimiento"></a>
### Sentimiento de noticias
Clasificar una noticia como positiva, negativa o neutral para un activo, con una magnitud de impacto estimada.

---

<a id="psicologia"></a>
## Psicología del mercado

Capa de contexto experimental ([spec §9.1](spec/fase-1-especificacion.md#psicologia-del-mercado)): se mide y se registra, pero no suma al puntaje hasta demostrar valor.

<a id="indice-sentimiento"></a>
### Índice de sentimiento
Un número de −1 (miedo extremo) a +1 (euforia extrema) que combina varias fuentes. Cada fuente se compara con sus propios últimos 3 años: importa si el dato de hoy es alto o bajo **para esa fuente**.
*Ejemplo:* si la volatilidad de hoy está entre las más altas de los últimos 3 años y el NASDAQ cayó mucho desde su máximo, el índice queda cerca de −1. Con −0,6 o menos se habla de "miedo extremo".

<a id="indicador-contrario"></a>
### Indicador contrario
Un dato que se lee **al revés**: cuando casi todos esperan lo mismo, el precio suele ir en contra, porque ya no queda quién compre (o venda). Es una hipótesis que se mide, no una regla.
*Ejemplo:* si el 80 % de los clientes de IG está largo en el NASDAQ, el indicador contrario espera que baje.

<a id="fomo"></a>
### FOMO
"Miedo a quedarse afuera": compras apuradas después de una suba fuerte. Acá se mide con el precio, sin opiniones: el cierre queda muy por encima de su media (3 ATR o más) y es el más alto de las últimas 20 velas.
*Ejemplo:* el NASDAQ sube 400 puntos en pocas horas y cierra en 27.150, muy lejos de su EMA20 de 1h en 26.800.

<a id="capitulacion"></a>
### Capitulación (pánico, clímax de venta)
El momento en que muchos venden a cualquier precio para salir. Se mide como el FOMO, pero hacia abajo, y se llama capitulación cuando además el índice de sentimiento está en miedo extremo.
*Ejemplo:* el NASDAQ cae 500 puntos en una tarde, cierra en el mínimo de 20 velas, y el índice marca −0,7.

<a id="rechazo"></a>
### Rechazo
El precio llega a un nivel importante y lo devuelven con fuerza: queda una mecha larga y la vela cierra lejos del nivel. Se arma con detectores que ya existen: [martillo](#martillo) o [estrella fugaz](#estrella-fugaz) sobre un [soporte](#soporte) o una [resistencia](#resistencia), o un [sweep](#sweep) que vuelve al rango.
*Ejemplo:* el NASDAQ baja hasta el soporte de 26.500, deja una mecha hasta 26.480 y cierra en 26.620.

<a id="distribucion"></a>
### Distribución
Los grandes inversores venden de a poco mientras el precio todavía se sostiene. Se mide contando días de cierre en baja con más volumen que el día anterior. Necesita **volumen real**, que los CFD de IG no informan, así que todavía no se calcula.
*Ejemplo:* 5 o más días así en las últimas 25 sesiones.

<a id="complacencia"></a>
### Complacencia
Calma excesiva: el mercado casi no se mueve y nadie espera sustos. Se mide con la volatilidad realizada en su 10 % más bajo de los últimos 3 años.
*Ejemplo:* el NASDAQ pasa semanas moviéndose menos de 0,5 % por día.

---

<a id="fondo"></a>
## Fondo del grupo

<a id="unidades"></a>
### Contabilidad por unidades (cuotapartes)
Forma de repartir de manera justa un fondo común cuando cada persona aporta montos distintos y en semanas distintas. El fondo se divide en **unidades**: cada aporte "compra" unidades al valor del día, y cada persona es dueña de lo que valen sus unidades.

<a id="nav"></a>
### Valor de la unidad (NAV)
NAV = valor total del fondo ÷ unidades emitidas.
*Ejemplo:*
- Semana 1: Ana aporta 100 € y Beto 50 €. El fondo tiene 150 €, la unidad vale 1 €, Ana tiene 100 unidades y Beto 50.
- El fondo gana 10% y pasa a 165 €. La unidad vale 1,10 €.
- Semana 2: Caro aporta 110 € y recibe 100 unidades (110 ÷ 1,10). No se lleva ganancia de la semana 1 porque no estaba.
- Ahora el P&L de Ana = 100 × 1,10 − 100 = **+10 €**; el de Beto = **+5 €**; el de Caro = **0 €**.

---

<a id="empresas"></a>
## Empresas (no aplica a índices)

Estas medidas son de la contabilidad de **una empresa**. Un índice (NASDAQ, DAX) no tiene EBITDA propio, así que el sistema **no las calcula ni las ingiere**. Quedan acá para entenderlas.

<a id="ebitda"></a>
### EBITDA
Ganancia de una empresa **antes** de intereses, impuestos, depreciación y amortización. Sirve para comparar cuánto genera el negocio en sí, sin mirar cómo se financia ni cuánto "gasta" en el desgaste contable de sus máquinas.
*Cómo se arma:* resultado operativo + depreciación + amortización. **No es una partida oficial** de las normas contables de EE. UU. (GAAP): la empresa no está obligada a publicarla, así que hay que armarla con las partidas que sí publica.
*Fuente oficial que haría falta:* los estados contables que cada empresa presenta ante la SEC (EDGAR, en formato XBRL), de donde se toman el resultado operativo y la depreciación y amortización.

<a id="ebitda-ajustado"></a>
### EBITDA ajustado
El EBITDA al que **la propia empresa** le saca gastos que considera "extraordinarios" (reestructuraciones, juicios, pagos en acciones, etc.). Cada empresa elige qué saca, así que **no es comparable entre empresas** y suele mostrar un número más lindo. Lo publica la empresa en sus presentaciones de resultados, no en los estados contables.

<a id="ebitda-normalizado"></a>
### EBITDA normalizado
El EBITDA corregido por **quien analiza** (no la empresa) para llevarlo a un año "típico": se sacan efectos que no se repiten y se ajustan los que dependen del ciclo. Lo usan sobre todo para valuar una empresa en una compra. No hay una fórmula única.

**Diferencia en una línea:** el EBITDA sale de los números contables; el ajustado lo decide la empresa; el normalizado lo decide el analista.

