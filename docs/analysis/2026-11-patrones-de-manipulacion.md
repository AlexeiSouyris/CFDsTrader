# Patrones de "manipulación" del mercado: qué son y cuáles podemos detectar

Issue #492 · 24-11-2026 · **informe de investigación para el dueño.** No cambia código, `config/`, la spec, la estrategia en uso, el riesgo, el runner ni el paper.

> **Solo para detectar, nunca para manipular.** Este informe explica qué patrones existen y cómo reconocerlos en los datos. Nada de esto es una recomendación de operar, y los resultados pasados no prometen ningún resultado futuro.

## Resumen en 5 puntos

1. **Hay dos cosas muy distintas que se llaman "manipulación".** Unas son **delitos** con una definición legal (spoofing, layering, wash trading, pump and dump, marcar el cierre, front running). Otras son **comportamientos normales del mercado** a los que el marketing de "smart money" les pone ese nombre (barridas de stops, spring y upthrust, trampas alcistas y bajistas, Judas swing, caza de stops en números redondos). Para estas últimas no hace falta un "manipulador": alcanza con que muchas personas pongan sus stops en los mismos lugares (Osler, 2003 y 2005).
2. **Los delitos no se pueden detectar con nuestros datos.** El spoofing y el layering necesitan el libro de órdenes completo (L3, "MBO"); el wash trading y el front running necesitan saber **quién** opera, algo que ningún dato público muestra. El pump and dump pasa en acciones chicas y en cripto, no en índices ni en oro. Para nuestro sistema no tienen uso práctico.
3. **Los comportamientos de mercado sí se pueden detectar con velas, y casi todo ya existe** en el crate `liquidity`: sweep de pivotes, mapa de niveles con estado *barrido* o *roto* (máximos y mínimos del día, de la semana, de las sesiones de Asia y Londres, máximos y mínimos iguales), FVG, BOS/CHoCH y order blocks. Lo único realmente nuevo que vale la pena es **el nivel de número redondo** y **el volumen de la barrida** (las velas de 1m de los futuros traen volumen).
4. **No hay evidencia seria de que estos patrones den ventaja después de costos.** No encontramos ni un paper revisado por pares sobre ICT o "smart money". Nuestro propio sweep ya perdió en los tres índices de EE. UU. con 15 años de datos (#461: −0,31 / −0,40 / −0,37 R por operación cuando el sweep es el único gatillo). La evidencia académica más fuerte (Osler) muestra un efecto de **horas**, en divisas, y dice que el precio **sigue** de largo al cruzar un nivel con stops, no que se dé vuelta.
5. **Propuesta:** dos familias para el programa #493 (cascada al cruzar un número redondo; barrida o ruptura de los niveles del día anterior, medida a horas y no a 15m), un estudio descriptivo de vencimientos de opciones, y el spring y upthrust se dejan a #491, que tiene que **reutilizar** `sweeps_at` y `levels` en vez de reescribirlos. Como el presupuesto de error del programa anterior se agotó (#486), cualquier familia necesita la decisión del dueño en #493.

## 1. Qué datos tenemos (y qué no)

| Dato | Qué tiene | Qué no tiene |
|---|---|---|
| Velas de 1m de futuros (Databento, esquema OHLCV-1m; ES, NQ, YM, RTY, FDAX, oro) | apertura, máximo, mínimo, cierre y **volumen** (contratos por minuto), 2011 → 2025 | órdenes, cancelaciones, quién opera, lado de cada operación |
| Velas de 15m y 1h armadas desde esas barras (`futures_candles`) | lo mismo, agrupado | lo mismo |
| Velas de 15m del CFD de IG | precios del CFD con spread | volumen real del mercado y libro de órdenes |

Con velas solo se ve **el resultado** del movimiento (dónde fue el precio), nunca **la intención** de quien lo movió. Por eso, en este informe "detectar" quiere decir "encontrar la huella en el precio", no "probar que alguien manipuló".

## 2. Lo que ya existe (no se duplica)

Consultado con graphify sobre el grafo del repo y leído en el código:

| Pieza | Dónde | Qué hace |
|---|---|---|
| Pivotes confirmados | `engine/crates/patterns/src/pivots.rs` (`confirmed_pivots`) | máximo o mínimo local que existe recién después de N velas de confirmación |
| Soportes, resistencias y rechazos | `patterns/src/levels.rs` (`levels_at`, `rejections_at`) | niveles por pivotes que se repiten |
| Doble techo y suelo | `patterns/src/double.rs` (`double_at`) | patrón chartista |
| Patrones de velas | `patterns/src/candles.rs` (`candle_patterns_at`) | martillo, estrella fugaz, envolvente... |
| **Sweep** | `liquidity/src/sweep.rs` (`sweeps_at`) | la vela de 15m pincha el último pivote de 1h confirmado y **cierra adentro** |
| **Mapa de niveles** | `liquidity/src/levels.rs` | EQH/EQL, máximo y mínimo del día anterior (PDH/PDL), de la semana (PWH/PWL) y de las ventanas de sesión (`asia` 00:00–07:00 y `london` 07:00–13:30 UTC). Cada nivel termina **barrido** (lo superó con la mecha y cerró adentro), **roto** (cerró afuera) o **vencido** |
| Rupturas de estructura | `liquidity/src/structure.rs` (`structure_breaks`) | BOS (sigue la tendencia de 1h) y CHoCH (posible giro) |
| FVG | `liquidity/src/fvg.rs` | huecos de 3 velas, toque y llenado |
| Order blocks | `liquidity/src/order_blocks.rs` | última vela contraria antes del impulso que rompió estructura |
| Desequilibrio | `liquidity/src/imbalance.rs` | velas de desplazamiento; **no** es flujo de órdenes |
| Rechazo | `liquidity/src/rejection.rs` | junta martillo/estrella + nivel, FVG, OB o sweep |
| Estadísticas | `liquidity/src/stats.rs` | eventos de FVG, gaps de apertura, agregados con IC de Wilson |

**Conclusión:** la "barrida de stops" ya está implementada **dos veces de forma complementaria** (contra pivotes en `sweep.rs` y contra niveles con nombre en `levels.rs`). Ningún patrón de este informe necesita un detector de barridas nuevo: necesita, como mucho, **un nivel nuevo** (número redondo) y **una medida nueva** (volumen).

## 3. Patrones ilegales (abuso de mercado)

### 3.1 Spoofing (órdenes falsas)

- **Qué es.** Alguien pone una orden grande de compra o de venta **que no piensa ejecutar**, para que los demás crean que hay mucho interés de ese lado. Cuando el precio se mueve, la cancela y opera del otro lado.
- **Fuente.** En EE. UU. es delito desde la ley Dodd-Frank de 2010: la sección 4c(a)(5)(C) de la Commodity Exchange Act prohíbe ofertar "con la intención de cancelar la oferta antes de su ejecución". Caso emblemático **en el futuro que usamos**: la CFTC contra Navinder Sarao por spoofing en el E-mini S&P 500 ([CFTC, comunicado 7486-16](https://www.cftc.gov/PressRoom/PressReleases/7486-16)). En la UE, el reglamento MAR (UE 596/2014), art. 12(2)(c): colocar, modificar o cancelar órdenes para dar señales falsas o desestabilizar el libro. Paper: Lee, E. J., Eom, K. S. y Park, K. S. (2013). *Microstructure-based manipulation: Strategic behavior and performance of spoofing traders*. Journal of Financial Markets, 16(2), 227–252 (Bolsa de Corea; ~0,81 % de las órdenes eran spoofing).
- **¿Ilegal?** Sí.
- **¿Detectable con nuestros datos?** **No.** Una vela no muestra órdenes que nunca se ejecutaron. Haría falta el libro completo orden por orden (**L3, esquema MBO** de Databento para CME, dataset `GLBX.MDP3`). Aun así, el libro es anónimo: se puede ver "una orden grande que aparece y se cancela", pero no probar la intención.
- **Costo de los datos.** Databento cobra el histórico por uso, en dólares por GB sin comprimir, con 125 USD de crédito gratis al registrarse ([FAQ oficial de precios](https://databento.com/docs/faqs/usage-pricing-and-data-credits)); el plan Standard para CME cuesta 179 USD por mes según su anuncio del 16-04-2025 ([blog oficial](https://databento.com/blog/introducing-new-cme-pricing-plans)). **No encontramos publicado el precio por GB del esquema MBO**: sale de la calculadora de [databento.com/pricing](https://databento.com/pricing), o, sin gastar, con el `--estimate` de nuestro importador (`cfds_intel/databento`, usa solo metadata). El MBO es muchísimo más pesado que las barras de 1 minuto, así que 15 años de ES serían caros; **no damos una cifra** porque no la verificamos.
- **Ya existe:** nada (y no debería: no tenemos el dato).
- **Definición medible (solo si algún día hubiera L3):** en el libro MBO, una orden en el lado A con tamaño ≥ `k` × la mediana de las órdenes de ese nivel, a ≤ `n` ticks del mejor precio, que se cancela sin ejecutarse en ≤ `s` segundos. La definición legal pide además que **la misma persona** opere del otro lado, y sin identidad de cuenta eso no se puede medir. Lo máximo medible es "órdenes grandes canceladas rápido", que también hacen los creadores de mercado legítimos.

### 3.2 Layering (varias capas de órdenes falsas)

- **Qué es.** Una variante del spoofing: en vez de una orden grande, se ponen **muchas** a precios escalonados para dar la sensación de una "pared".
- **Fuente.** Misma base legal que el spoofing (CEA 4c(a)(5)(C); MAR art. 12(2)(c)). El reglamento delegado (UE) 2016/522 da indicadores de este tipo de conducta; no lo verificamos letra por letra.
- **¿Ilegal?** Sí. **¿Detectable?** No, mismo motivo (hace falta L3 o al menos L2 de 10 niveles, esquema MBP-10). **Ya existe:** nada. **Definición:** la misma que el spoofing, con `m` ≥ 3 órdenes del mismo lado en niveles consecutivos que se cancelan juntas.

### 3.3 Wash trading (operar contra uno mismo)

- **Qué es.** Comprar y vender a la vez, entre cuentas propias, para inflar el volumen y que el activo parezca más activo de lo que es. No cambia el dueño real de nada.
- **Fuente.** MAR art. 12(1)(a) y su anexo I, sección A (indicadores de señales falsas, entre ellos las operaciones sin cambio de titular). Paper: Cong, L. W., Li, X., Tang, K. y Yang, Y. (2023). *Crypto wash trading*. Management Science, 69(11), 6427–6454 (en exchanges de cripto no regulados, más del 70 % del volumen informado era falso).
- **¿Ilegal?** Sí. **¿Detectable con nuestros datos?** **No.** Hace falta saber que las dos puntas son de la misma persona; ningún dato público lo dice. En futuros de CME, con cámara compensadora y vigilancia, es un problema chico frente al volumen; afecta sobre todo a cripto.
- **Ya existe:** nada. **Definición:** no proponemos una; no hay forma honesta de medirlo con datos anónimos.

### 3.4 Pump and dump (inflar y vender)

- **Qué es.** Se infla el precio de un activo chico con noticias falsas o promoción ("¡va a subir!"), y cuando la gente compra, los promotores venden. Después el precio se cae.
- **Fuente.** SEC, [Investor.gov: Pump and Dump Schemes](https://www.investor.gov/protect-your-investments/fraud/types-fraud/pump-and-dump-schemes). En la UE, MAR art. 12(2)(d) (opinar en medios teniendo una posición sin declararla).
- **¿Ilegal?** Sí. **¿Detectable con nuestros datos?** **No aplica:** pasa en acciones de muy baja capitalización y en cripto. Un índice como el S&P 500 o el oro no se puede inflar así.
- **Ya existe:** nada. **Definición:** no proponemos una.

### 3.5 Marcar el cierre (marking the close)

- **Qué es.** Comprar o vender fuerte **en los últimos minutos** de la sesión para dejar el precio de cierre donde conviene (por ejemplo, para valuar una cartera o un derivado).
- **Fuente.** MAR art. 12(2)(b) (operar en la apertura o el cierre engañando a quien usa esos precios). Papers: Comerton-Forde, C. y Putniņš, T. J. (2011). *Measuring closing price manipulation*. Journal of Financial Intermediation, 20(2), 135–158 (arman un índice de probabilidad de manipulación del cierre con casos juzgados); Carhart, M. M., Kaniel, R., Musto, D. K. y Reed, A. V. (2002). *Leaning for the tape: Evidence of gaming behavior in equity mutual funds*. Journal of Finance, 57(2) (fondos que suben sus acciones al cierre de trimestre; cita de memoria, no la volvimos a verificar en esta sesión).
- **¿Ilegal?** Sí, cuando hay intención.
- **¿Detectable con nuestros datos?** **Solo una huella aproximada**, y en índices grandes es muy poco probable que exista. Con las barras de 1m de los futuros se puede medir el movimiento de los últimos minutos antes del cierre del contado (16:00 ET para ES/NQ/YM; 17:30 CET para FDAX) y si se **revierte** en la apertura siguiente, que es la huella que usan Comerton-Forde y Putniņš. Les falta el lado y el operador.
- **Ya existe:** nada específico. **Relacionado y cerrado:** la familia B de #486 (momentum de la última media hora) ya midió el final del día en los mismos datos y **no pasó**.
- **Definición medible (descriptiva):** `r_fin = ln(C[16:00 ET] / C[16:00 ET − m min])`, con `m` en config (p. ej. 10). Evento si `|r_fin| ≥ z × σ_fin`, donde `σ_fin` es la desviación de `r_fin` de los `N` días **anteriores** (sin look-ahead). Huella: `r_rev = ln(O[día+1] / C[16:00 ET])` de signo contrario. Se mide la proporción de reversiones con IC de Wilson (`cfds_indicators::wilson`), en días de fin de mes y trimestre frente al resto. Sin uso para operar.

### 3.6 Front running (adelantarse a la orden de un cliente)

- **Qué es.** Un intermediario que sabe que un cliente va a mandar una orden grande opera antes para aprovechar el movimiento que va a causar.
- **Fuente.** FINRA, [Regla 5270, *Front Running of Block Transactions*](https://www.finra.org/rules-guidance/rulebooks/finra-rules/5270). En la UE es abuso de información privilegiada: MAR art. 7(1)(d) (información sobre órdenes pendientes de clientes).
- **¿Ilegal?** Sí. **¿Detectable con nuestros datos?** **No.** Hace falta saber qué órdenes de clientes tenía el intermediario; es información interna. **Ya existe:** nada. **Definición:** no proponemos una.

## 4. Comportamientos de mercado que se suelen llamar "manipulación"

Ninguno de estos es ilegal. La palabra "manipulación" sugiere que alguien lo hace a propósito; la explicación académica más sencilla es otra: **los stops de mucha gente están juntos** (debajo de un mínimo, arriba de un máximo, cerca de un número redondo), y cuando el precio llega ahí, esas órdenes se ejecutan solas y empujan el precio un poco más (Osler, 2005).

### 4.1 Barrida de stops o liquidity sweep

- **Qué es.** El precio pasa apenas por encima de un máximo (o por debajo de un mínimo) donde había stops, los activa, y vuelve. En la vela se ve como una **mecha larga** más allá del nivel y un cierre de vuelta adentro.
- **Fuente.** Osler, C. L. (2005). *Stop-loss orders and price cascades in currency markets*. Journal of International Money and Finance, 24(2), 219–241: las órdenes stop producen "cascadas" de precio que se refuerzan solas; el efecto es significativo **a horas, no a días** ([resumen en RePEc](https://ideas.repec.org/p/fip/fednsr/150.html)). Ojo: Osler muestra **cascadas** (el precio sigue), no que después se dé vuelta.
- **¿Ilegal?** No. **¿Detectable?** **Sí, y ya está hecho.**
- **Ya existe:** `sweeps_at` (contra pivotes de 1h) y el estado *barrido* de `levels` (contra niveles con nombre). Se usa como gatillo en la estrategia y ya se midió (#461, ver §6).
- **Definición medible:** la existente. Barrida alcista de un nivel `P` conocido al cierre de `t−1`: `H[t] > P` y `C[t] ≤ P` (en `levels`) o, contra pivotes, `L[t] < P` y `C[t] > P` (en `sweep.rs`). Ya es sin look-ahead: el nivel existe antes y la vela está cerrada. **Lo único que se podría sumar** es una medida de profundidad y volumen (ver 4.6).

### 4.2 Spring y upthrust (Wyckoff)

- **Qué es.** Del método de Richard Wyckoff (años 1930). Después de un período de precio lateral (un "rango"), el *spring* es una caída rápida por debajo del piso del rango que vuelve enseguida adentro; el *upthrust* es lo mismo arriba del techo. Se interpreta como que "alguien grande" sacó los stops antes de mover el precio para el otro lado.
- **Fuente.** **No encontramos un paper revisado por pares** que defina o pruebe spring/upthrust. Las fuentes son libros de practicantes que siguen el curso original de Wyckoff. Lo decimos tal cual.
- **¿Ilegal?** No. **¿Detectable?** Sí, con velas.
- **Ya existe:** en la práctica es una **barrida del borde de un rango**. Las piezas están: `sweeps_at` / estado *barrido* de `levels` para la barrida, y pivotes confirmados para los bordes. **Lo implementa #491**; este informe no lo duplica. Recomendación para #491: definir el "rango" con pivotes confirmados de `patterns` y la barrida con la misma regla de `levels`/`sweep.rs`, sin un detector de barridas propio.
- **Definición medible (propuesta para #491):** rango en `t` = las últimas `n` velas cerradas (config) con `(máx − mín) ≤ a × ATR` y al menos `k` pivotes confirmados tocando cada borde (tolerancia `b × ATR`). Spring en `t`: `L[t] < piso − c × ATR` y `C[t] > piso`. Confirmación: el spring vale recién al cierre de `t + j` si ninguna vela en `(t, t+j]` cerró debajo de `L[t]`. Upthrust: el espejo.

### 4.3 Trampas alcistas y bajistas (bull trap y bear trap)

- **Qué es.** Una ruptura falsa: el precio cierra por encima de una resistencia (parece que va a subir, entran compradores) y enseguida vuelve abajo, dejando "atrapados" a los que compraron. La *bear trap* es el espejo.
- **Fuente.** No encontramos una definición académica propia. Se relaciona con Osler (2003), que explica por qué los precios rebotan en soportes y resistencias y por qué se aceleran cuando los cruzan: Osler, C. L. (2003). *Currency orders and exchange rate dynamics: An explanation for the predictive success of technical analysis*. Journal of Finance, 58(5), 1791–1819.
- **¿Ilegal?** No. **¿Detectable?** Sí.
- **Ya existe:** casi todo. La diferencia con la barrida es que en la trampa **la vela cierra afuera** (en `levels` eso es estado *roto*) y recién después vuelve. `structure_breaks` marca la ruptura (BOS/CHoCH).
- **Definición medible:** nivel `P` conocido antes de `t`. Ruptura en `t`: `C[t] > P` (= estado *roto* de `levels`). Trampa confirmada en `t + j` (`1 ≤ j ≤ J`, config): la primera vela que cierra `C[t+j] < P − d × ATR[t+j]`. La señal existe recién al cierre de `t + j`, nunca antes. Bear trap: el espejo. Lo nuevo sería solo el seguimiento después de *roto*, que hoy `levels` no guarda (el estado no cambia más una vez roto).

### 4.4 Judas swing y stop hunt (ICT)

- **Qué es.** Término de ICT ("Inner Circle Trader", un educador de internet). La idea: al abrir Londres o Nueva York, el precio hace primero un movimiento **en la dirección equivocada** (el "Judas") para barrer los stops del máximo o mínimo de la sesión asiática, y después va para el lado "real".
- **Fuente.** **Ninguna seria.** No encontramos un paper revisado por pares sobre ICT ni sobre "smart money concepts". Es material de cursos y videos.
- **¿Ilegal?** No. **¿Detectable?** Sí, porque en la práctica es una barrida de un nivel de sesión en una franja horaria.
- **Ya existe:** `levels` ya calcula el máximo y el mínimo de la ventana `asia` (00:00–07:00 UTC) y su estado *barrido*. Lo único que falta es **filtrar por la hora** en que ocurre la barrida.
- **Definición medible:** sea `AH`/`AL` el máximo/mínimo de la ventana `asia` (existe desde las 07:00 UTC). Judas bajista en `t`, con `t` dentro de las primeras `w` velas de `london` (config): `H[t] > AH` y `C[t] ≤ AH` (estado *barrido* de `levels`). Judas alcista: el espejo con `AL`. Sin código nuevo de detección: es una consulta sobre `liquidity_levels` y `liquidity_level_events` ya guardados.

### 4.5 Caza de stops en números redondos

- **Qué es.** La gente pone sus objetivos de ganancia **en** los números redondos (5000, 5100...) y sus stops **un poco más allá**. Por eso el precio tiende a frenar en el número redondo y, si lo cruza, a acelerar.
- **Fuente.** Osler (2003), citado arriba: con datos reales de órdenes de un banco, las órdenes de toma de ganancia se juntan **en** los números redondos y los stops **justo después** de ellos; eso explica rebotes en el nivel y aceleración al cruzarlo. Osler (2005) mide la cascada. **Las dos son en divisas**, no en índices.
- **¿Ilegal?** No. **¿Detectable?** **Sí, y es lo más nuevo de este informe:** el mapa de liquidez no tiene números redondos.
- **Ya existe:** nada. Sería un **tipo de nivel más** dentro de `levels` (con su estado *barrido*/*roto* ya resuelto), no un módulo nuevo.
- **Definición medible:** paso `S` por instrumento en `config/` (por ejemplo US500 50 puntos, US100 100, US30 500, DE40 100, oro 10; a fijar antes de mirar los datos). Nivel redondo `R = k × S`. Cruce alcista en `t`: `C[t−1] < R` y `C[t] ≥ R + e × ATR[t]`. Hipótesis de Osler (cascada): el retorno de `t+1` a `t+h` en la dirección del cruce es **mayor** que en un control: los mismos cruces en niveles **no** redondos `R + S/2`. El control es lo que separa "número redondo" de "cualquier ruptura".

### 4.6 Volumen de la barrida (medida nueva, no patrón)

- **Qué es.** Si una barrida activó muchos stops, debería verse **más volumen** en esa vela. Es la única huella de "órdenes ejecutadas" que dan las velas.
- **Fuente.** Se deriva de Osler (2005): una cascada de stops implica ejecuciones. No encontramos un paper que lo pruebe en velas de índices.
- **Detectable:** sí, solo con las barras de futuros (IG no da volumen real).
- **Ya existe:** `imbalance` mide desplazamiento por precio, **no** volumen.
- **Definición medible:** `vz[t] = (V[t] − media(V, N)) / desvío(V, N)`, con la media y el desvío de las `N` velas **del mismo horario** de días anteriores (el volumen tiene un patrón fuerte por hora del día). Se usa como **contexto** de una barrida existente, no como gatillo.

### 4.7 Pinning y gamma (vencimiento de opciones)

- **Qué es.** El día que vencen las opciones, el precio de una acción tiende a terminar **cerca de un precio de ejercicio** (el "strike"). Una explicación es que quienes venden opciones se cubren comprando y vendiendo el activo, y eso empuja el precio hacia el strike. Fuera del vencimiento, esa cobertura (la "gamma") puede amplificar o frenar los movimientos.
- **Fuente.** Ni, S. X., Pearson, N. D. y Poteshman, A. M. (2005). *Stock price clustering on option expiration dates*. Journal of Financial Economics, 78(1), 49–87 (en acciones con opciones, lo atribuyen a la cobertura de los creadores de mercado y **también** a manipulación de operadores propios). Ni, S. X., Pearson, N. D., Poteshman, A. M. y White, J. (2021). *Does option trading have a pervasive impact on underlying stock prices?* Review of Financial Studies, 34(4), 1952–1986. Baltussen, G., Da, Z., Lammers, S. y Martens, M. (2021). *Hedging demand and market intraday momentum*. Journal of Financial Economics, 142(1), 377–403 (la cobertura de gamma explica el momentum de la última media hora en más de 60 futuros).
- **¿Ilegal?** La cobertura no. Mover el precio a propósito hacia un strike sí sería manipulación (Ni et al., 2005, encuentran evidencia de ambas cosas en acciones).
- **¿Detectable con nuestros datos?** **Parcialmente.** Sabemos **cuándo** vencen (tercer viernes de cada mes; el calendario de bolsa está en `market-data`), pero **no** el interés abierto por strike ni la gamma, que son los que dicen dónde y cuánto debería pegarse el precio. Esos datos vienen de la OCC/Cboe o de proveedores como OptionMetrics; **no verificamos sus precios**. Para el S&P 500, los strikes van cada 5 puntos, así que "cerca de un strike" casi siempre se cumple: la prueba con solo velas es débil.
- **Ya existe:** nada específico. **Relacionado y cerrado:** la hipótesis de Baltussen et al. ya se probó como familia B de #486 y **no pasó** (−0,01 / −0,01 / −0,03 R antes de costos).
- **Definición medible (descriptiva):** en días de vencimiento mensual frente a los demás viernes, (a) el rango de la última hora antes del cierre del contado en múltiplos de ATR y (b) la distancia del cierre al múltiplo de 25 puntos más cercano, comparada con la distribución uniforme que daría el azar. Sin uso para operar.

## 5. Tabla resumen

| Patrón | Tipo | Fuente seria | ¿Detectable con nuestras velas? | Qué falta | Qué ya existe | Prioridad |
|---|---|---|---|---|---|---|
| Spoofing | ilegal | CEA 4c(a)(5)(C); MAR 12(2)(c); Lee et al. 2013 | no | L3 (MBO), y aun así anónimo | nada | no |
| Layering | ilegal | igual que spoofing | no | L2/L3 (MBP-10/MBO) | nada | no |
| Wash trading | ilegal | MAR 12(1)(a) y anexo I; Cong et al. 2023 | no | identidad de cuentas (no pública) | nada | no |
| Pump and dump | ilegal | SEC Investor.gov; MAR 12(2)(d) | no aplica (índices, oro) | — | nada | no |
| Marcar el cierre | ilegal | MAR 12(2)(b); Comerton-Forde y Putniņš 2011 | solo una huella aproximada (1m de futuros) | lado y operador | nada; familia B (#486) cerrada | baja (descriptivo) |
| Front running | ilegal | FINRA 5270; MAR 7(1)(d) | no | órdenes de clientes (interno) | nada | no |
| Barrida de stops | mercado | Osler 2005 | sí | — | `sweeps_at`, estado *barrido* de `levels` | ya hecho; reestudiar a horas (familia M2) |
| Spring / upthrust | mercado | sin paper revisado | sí | — | piezas en `sweep`, `levels`, `pivots` | la hace #491 |
| Trampa alcista/bajista | mercado | sin definición académica; ligado a Osler 2003 | sí | seguimiento después de *roto* | estado *roto* de `levels`, `structure_breaks` | media (dentro de M2) |
| Judas swing (ICT) | mercado | ninguna seria | sí | filtro horario | `levels` ventana `asia` + estado *barrido* | baja (consulta) |
| Números redondos | mercado | Osler 2003 y 2005 | sí | tipo de nivel nuevo | nada | **alta (familia M1)** |
| Volumen de la barrida | medida | derivada de Osler 2005 | sí, solo futuros | — | `imbalance` (no es volumen) | media (contexto) |
| Pinning / gamma | mercado | Ni et al. 2005 y 2021; Baltussen et al. 2021 | parcial | interés abierto y gamma por strike | familia B (#486) cerrada | baja (descriptivo) |

## 6. Evidencia honesta: ¿dan ventaja después de costos?

**Corta: no hay evidencia de que la den, y la nuestra dice que no.**

- **Nuestro propio resultado.** En el diagnóstico de 15 años de US500, US100 y US30 (#461, `docs/analysis/2026-11-diagnostico-us-futuros.md`), las operaciones donde el sweep es el único gatillo dieron **−0,31 R, −0,40 R y −0,37 R** por operación, con 60–70 operaciones por índice. Combinado con rechazo en un nivel: −0,23 / −0,10 / −0,12 R. Ninguna combinación con sweep es positiva con IC que no cruce el cero. Y antes de costos, la estrategia entera de 15m está en ≈ 0 R; los costos se llevan 0,10–0,18 R por operación.
- **Lo académico dice algo distinto de lo que vende el marketing.** Osler encuentra que cruzar un nivel con stops **acelera** el precio en la misma dirección por unas horas (cascada). El relato de "smart money" dice lo contrario: que después de la barrida el precio **se da vuelta**. Nuestro sweep apuesta a la vuelta y pierde, lo que es coherente con Osler.
- **El análisis técnico en general no sobrevive a los costos.** Bajgrowicz, P. y Scaillet, O. (2012). *Technical trading revisited: False discoveries, persistence tests, and transaction costs*. Journal of Financial Economics, 106(3), 473–491: con más de un siglo del Dow Jones y corrección por búsqueda de datos, no se podía elegir de antemano qué reglas iban a funcionar, y aun las mejores en la muestra se borran con costos bajos.
- **ICT / smart money / Wyckoff:** no encontramos **ningún** estudio revisado por pares que muestre ventaja. Lo que circula son capturas de pantalla elegidas después del hecho, que es justo el autoengaño que prohíbe la sección 5b de `CLAUDE.md`.
- **Gamma:** la única hipótesis con buena evidencia académica (Baltussen et al.) ya se probó en nuestros datos (familia B, #486) y perdió.
- **Costos.** Con un stop de 1–1,5 ATR de 15m, el costo de IG es 0,11–0,14 R por operación en US500 (#462). Cualquier patrón que se opere en 15m arranca con esa desventaja. Por eso las familias de abajo se miden a **horas** y con un R más ancho, no a 15m.

## 7. Cómo se agregarían al sistema

Tres formas, de menos a más riesgo de autoengaño:

1. **Como contexto (informativo).** Se calcula, se guarda y se muestra en el gráfico, **sin** sumar al puntaje. Así entraron el mapa de liquidez y las estadísticas de FVG (#231). Candidatos: nivel de número redondo dentro de `levels`, `vz` (volumen de la barrida), Judas swing como consulta. No gasta presupuesto de error porque no decide nada.
2. **Como filtro de la estrategia actual.** **No se recomienda.** #461 ya miró ~300 cortes y no hay ventaja antes de costos que filtrar (#462 §3, "familias que se descartan").
3. **Como familia de investigación en #493**, con hipótesis medible, pre-registro commiteado antes de correr y su parte del presupuesto de error. Esta es la única forma de saber si algo sirve.

### Familias propuestas para #493 (no se implementan acá)

**M1. Cascada al cruzar un número redondo** (Osler 2003 y 2005).
- Hipótesis: después de que la vela de 15m de futuros cierra cruzando un número redondo (`S` fijado antes de mirar), el retorno de las `h` horas siguientes (p. ej. 2 h) en la dirección del cruce es mayor que después de cruzar un nivel intermedio `R + S/2`.
- Prueba: diferencia de medias con IC por bloques; si pasa, una regla operable (entrada en `t+1`, stop del otro lado del nivel a `x` ATR, salida a `h` horas) con costos completos.
- Por qué es distinta de lo ya probado: el nivel es nuevo (no es un pivote), la apuesta es de **continuación** (no de vuelta) y el horizonte es de horas.
- Riesgo: el efecto de Osler es en divisas con datos de hace más de 20 años; puede no existir en índices.

**M2. Barrida frente a ruptura de los niveles del día anterior, a horas** (Osler 2005; estados ya guardados en `liquidity_level_events`).
- Hipótesis: después de que el precio **rompe** (cierra afuera) el máximo o mínimo del día anterior, el retorno a `h` horas sigue en la dirección de la ruptura más que después de una **barrida** (mecha y cierre adentro). Incluye la trampa (§4.3) como tercer grupo.
- Primero es un **estudio de eventos descriptivo** con lo ya guardado (no gasta presupuesto); solo si muestra una diferencia de tamaño mayor que el costo se pre-registra como familia.
- Por qué es distinta: usa niveles diarios con nombre y horizonte de horas, no el sweep de pivotes de 1h en 15m que ya perdió.

**Estudio descriptivo (sin familia): vencimientos de opciones y marcar el cierre** (§3.5 y §4.7). Solo para saber si existe la huella; no se opera.

**No entran:** spoofing, layering, wash trading, pump and dump y front running (sin datos y sin uso); Judas swing (sin fuente seria; queda como consulta de contexto); spring y upthrust (los hace #491; si #491 muestra algo prometedor, puede proponerse como familia después).

**Presupuesto de error:** la ronda 2 (#486) agotó el 5 % del programa de #466. M1 y M2 necesitan que el dueño apruebe el programa nuevo de #493 y les asigne su parte.

## 8. Qué decide el dueño

1. Si M1 y M2 entran al programa de #493 (con su pre-registro y presupuesto).
2. Si se agrega el número redondo como nivel **informativo** a `levels` (Issue aparte, solo contexto).
3. Si vale la pena pedir una estimación de costo del MBO de ES a Databento con `--estimate` (gratis) **solo** para saber el número. No recomendamos comprarlo: no hay un uso claro para un sistema que opera CFDs.

## Límites de este informe

- Las fuentes legales y los papers se verificaron por búsqueda (registros de RePEc, editoriales y reguladores), no siempre leyendo el texto completo. Carhart et al. (2002) se cita de memoria.
- No se corrió ningún dato nuevo: los números de §6 son de informes ya mergeados (#461, #462, #486).
- No se consultó IG ni se gastó cupo.
- Los precios de datos de Databento son los publicados a la fecha de sus páginas oficiales; el precio por GB del MBO no estaba publicado en ellas.
