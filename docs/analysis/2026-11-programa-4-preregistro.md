# Programa 4: pre-registro (borrador). Reglas de Gianluca Bruni en el CFD US100

> **Estado: borrador, no congelado.** Issue #561 · 25-11-2026 · escrito por un agente de IA (Claude Code) para que lo revise el dueño.
>
> Mientras sea borrador **no se corre nada** con estas reglas y **no se mira ningún resultado**. Se congela recién cuando el dueño cierre los "Puntos abiertos antes de congelar" (§12): congelar es commitear este archivo con su fecha y hora y su SHA-256 (en un `.sha256` al lado y en el PR), antes de que corra ningún código con estas reglas. Cambiar algo después de correr invalida la hipótesis y cuenta como una prueba nueva.
>
> Base: el informe `docs/research/2026-11-gianluca-bruni.md` y el análisis del video `docs/research/videos/2026-11-25-emre-vs-gianluca.md`. Método y formato: el pre-registro del programa Nasdaq (`2026-11-programa-nasdaq-preregistro.md`) y el de la ronda 2 (`2026-11-ronda2-preregistro.md`).

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real (`CLAUDE.md` §5): aunque una hipótesis pase, queda en simulación.

## 0. Aprobación y presupuesto

- **Aprobación del dueño** (Alexei), 25-11-2026, registrada en #561, textual: "dale, apruebo el margen del 5 %". Abre el programa 4 con un presupuesto **nuevo** de falsos positivos del **5 %**.
- **Reparto propuesto** (lo decide el dueño antes de congelar; Bonferroni, unilateral):

| Hipótesis | Qué prueba | Nivel |
|---|---|---|
| H4.1 | Estrategia A **literal**: dos velas de 15 m con cuerpo ≥ 15 puntos a favor de la estructura | 1,25 % (p < 0,0125) |
| H4.2 | Estrategia A **normalizada**: cuerpo ≥ k × ATR de 15 m, con k fijado antes (§5) | 1,25 % (p < 0,0125) |
| H4.3 | Estrategia C ("de campeonato"): vuelta al área de valor de la sesión anterior, cierre sobre el VAH y el máximo anterior, volumen "eficiente", 1:2 | 1,25 % (p < 0,0125) |
| Reserva | Candidata: "saltear la operación que sigue a una pérdida" (§8) | 1,25 %, sin gastar |

- **La reserva no se gasta en variantes** de H4.1–H4.3 (otros umbrales, otros objetivos, otros horarios). Solo se puede usar con un pre-registro nuevo y fechado, como se hizo con la reserva del programa 2.
- **Cantidad de pruebas para el Sharpe deflactado:** el N del registro **al congelar**. Hoy es **356** (`config/research.toml`, `[rule_nasdaq]`). Si se congelan H4.1–H4.3 y nada más se congela antes, **N = 359** (360 si el programa 3 se congela primero, 360 o 361 si además se asigna la reserva). Referencia con N = 100 y N = 1000, sin voto.
- **Ojo con el total del proyecto:** el programa 3 (#513, borrador) también pide un 5 % propio. Si los dos siguen vivos, la tasa total de falsos positivos del proyecto sube (punto abierto 1).
- Para escribir este borrador **no se leyó ningún precio ni ninguna fila de la base.**

## 1. Lo que viene de los programas anteriores sin cambios

- **Período:** la concatenación de las **51 ventanas de validación** del walk-forward de `config/data_split.toml` (24 meses de entrenamiento, 3 de validación, paso 3): **01-02-2013 → 01-11-2025** (153 meses). Las reglas no tienen parámetros que se estimen con los datos (k de H4.2 se fija antes, §5), así que la parte de entrenamiento no se usa, salvo para el calentamiento de los indicadores.
- **El fuera de muestra (desde el 01-11-2025) no se carga, no se mira y no se usa.** Usarlo pide la aprobación del dueño, una sola vez, para un solo candidato congelado.
- **Costos** (las funciones de `engine/crates/backtest/src/costs.rs`, los mismos supuestos que el programa Nasdaq §1):
  - spread **1,6 puntos** en la sesión de contado (09:30–16:00 ET; `config/instruments.toml`); **2,5 puntos** fuera de ella (`docs/research/ig-spreads-2026-11.md`). Con las reglas de este programa todas las ejecuciones caen dentro de la sesión de contado;
  - slippage por ejecución = 0,1 × spread + 1 tick (0,1 punto) (`config/backtest.toml`); un stop que se ejecuta paga el mismo slippage;
  - comisión 0 (índices de IG);
  - financiación de IG noche por noche menos la base del futuro (#470, ADR 0015), con los **dos escenarios** EUR y USD de #421. Como todas las posiciones se cierran el mismo día (§2.4), se espera que sea 0; se calcula igual;
  - **estrés:** todos los costos × 1,5 (criterio 4); × 2 solo se informa.
- **Cuenta y riesgo** (igual que la ronda 2 §2.2): cuenta de **100.000 €**, nueva en cada ventana de validación. Toda operación pasa por `cfds_risk::check` con `config/risk.toml` **sin cambios**: 1 % de riesgo por operación, pérdida diaria máxima 3 %, caída máxima 15 % con el kill switch del backtest, stop entre 0,5 y 3 ATR(14) de 15 m de la vela de la decisión, apalancamiento máximo 10:1. Lo que el riesgo rechaza **no se opera** y se informa por motivo. Tamaño continuo (el informe cuenta aparte cuántas operaciones no se habrían podido abrir con 10.000 € por el mínimo de IG).
- **Vara de comparación (x):** comprar y mantener el CFD US100 con los mismos costos y los dos escenarios (`buy_and_hold_returns`, una cuenta nueva por ventana), igual que el programa Nasdaq.
- Retornos mensuales en UTC. Estadística en `f64`, dinero en `Decimal`.

## 2. Datos, horarios y reglas comunes

### 2.1 Instrumento y datos

- **Costos y P/L:** el **CFD US100 de IG** (EEUU Tech 100, 1 € por punto; `config/instruments.toml`).
- **Precio:** velas de **1 minuto del futuro NQ** (`futures_bars_1m`, símbolo `NQ.c.0`, GLBX.MDP3, Databento, desde 2011), armadas en velas de **15 minutos** con la preparación común (`examples/common/prepare.rs`: validación, horario de la spec, vencimientos del calendario, base del futuro), `--fuente futuros`, conexión de solo lectura. Las velas se identifican por su hora de inicio.
- **Volumen (solo H4.3):** el volumen real del NQ de las mismas velas de 1 minuto. El CFD no tiene volumen propio.
- **Diferencia entre el CFD y el futuro:**
  - El CFD de contado de IG sigue al **índice**; el futuro NQ cotiza el índice **más la base** (intereses menos dividendos hasta el vencimiento). Dentro de un día la base casi no cambia, así que los **movimientos** en puntos (cuerpos de velas, ATR, distancia al stop y al objetivo) son casi iguales. Lo que cambia es el **nivel**, que en estas reglas no se usa salvo como diferencia.
  - La **financiación** del CFD sí existe y el futuro ya la tiene en el precio: por eso se descuenta la base (#470). Acá casi no importa porque no se cruza la noche.
  - El **horario** es parecido (el NQ cotiza casi 23 h y el CFD también), pero las reglas operan solo en la sesión de contado.
  - El **volumen** es del futuro, no de IG: es el volumen real del mercado, que es lo que mira Bruni.
  - En los **vencimientos** el futuro salta de contrato. La preparación común cierra lo abierto y reinicia el calentamiento (#267, #269). Acá además se **reinicia el estado de la estructura, del ATR, del percentil y del perfil de volumen** (§2.5).

### 2.2 Zona horaria y sesión

- "ET" = hora local de Nueva York (`America/New_York`), con su horario de verano; cada hora se convierte a UTC en la fecha de ese día.
- **Sesión de contado** = la de XNYS en `config/sessions.toml` (09:30–16:00 ET, con feriados y cierres tempranos).
- **Velas de la sesión** = las velas de 15 m que empiezan entre la apertura y 15 minutos antes del cierre (09:30, 09:45, …, 15:45 ET en un día normal).

### 2.3 Decisión y ejecución (sin mirar el futuro)

- La **decisión** se toma al **cierre** de la vela `t` con información de velas **cerradas** hasta `t`. La orden se ejecuta en la **apertura** de la vela `t + 1` (`CLAUDE.md` §5b).
- `t` y `t + 1` tienen que ser **consecutivas sin hueco**. Las velas que usa una regla (por ejemplo `t − 1` y `t`) también. Si falta alguna, no hay señal.
- La decisión tiene que estar **permitida** por las marcas de entrada de la preparación común (spec §2, `config/trading_window.toml`: no en los primeros 15 minutos de la sesión, no en los últimos 30 minutos de la semana) y además la vela de ejecución tiene que empezar **antes de las 15:30 ET** (o 30 minutos antes de un cierre temprano).

### 2.4 Gestión de la posición (común a todas)

- **Una posición a la vez.** Mientras hay una abierta no se toma otra señal.
- **Stop y objetivo fijos** desde la entrada; no se mueven.
- En cada vela, desde la de la entrada inclusive, en este orden: (1) si la **apertura** ya saltó el stop, sale a la apertura; (2) si el mínimo o el máximo **tocó el stop**, sale al stop; (3) si tocó el **objetivo**, sale al objetivo; (4) cierre forzado.
- **Si una misma vela de 15 m toca el stop y el objetivo, se supone el stop** (conservador, como la spec §8). Como descriptivo se informa el resultado resolviendo esos casos con las velas de 1 minuto.
- **Cierre forzado del día:** en la apertura de la **última vela de la sesión** (15:45 ET; 12:45 ET en un cierre temprano). Ninguna posición cruza la noche. También cierra lo que fuerce la preparación (vencimiento).
- Si en la ejecución el precio de entrada queda **del lado equivocado del stop** (largo con entrada ≤ stop, corto con entrada ≥ stop), no se opera.
- **Sin piramidación** y **sin riesgo variable**: fuera de alcance (§9).
- Largos y cortos: los cortos son el espejo exacto de los largos.

### 2.5 Vencimientos

En cada cambio de contrato del calendario de vencimientos, además de lo que ya hace la preparación, se **reinicia** todo el estado de §3, §4 y §6 (zonas, rupturas, ATR, percentil, perfiles). No hay señal hasta completar de nuevo el calentamiento de cada cosa. Así el salto de precio entre contratos no se lee como una ruptura.

## 3. Definiciones objetivas de la estructura (comunes a H4.1, H4.2 y H4.3)

Se calculan sobre **todas** las velas de 15 m del NQ (las casi 23 h del día), no solo las de la sesión de contado. Son funciones puras: con las mismas velas dan el mismo resultado, y lo que se calculó para una vela cerrada no cambia cuando llegan velas nuevas.

- **Cuerpo** = |cierre − apertura|. **Mecha superior** = máximo − máx(apertura, cierre). **Mecha inferior** = mín(apertura, cierre) − mínimo.
- **Vela alcista** si cierre > apertura; **bajista** si cierre < apertura; si son iguales, ninguna.
- **Vela válida** = cuerpo **estrictamente mayor** que la mecha más larga: cuerpo > máx(mecha superior, mecha inferior).
- **Zona (caja):** se arma vela a vela.
  - Una zona empieza en una vela `s` con techo `H` = máximo de `s` y piso `L` = mínimo de `s`. Cuenta sus velas válidas alcistas (`n_alc`) y bajistas (`n_baj`), incluida `s`.
  - Para cada vela `i` cerrada siguiente:
    - **Ruptura alcista** si `n_baj ≥ 3` y cierre de `i` > `H` + δ.
    - **Ruptura bajista** si `n_alc ≥ 3` y cierre de `i` < `L` − δ.
    - Si hay ruptura, se registra (dirección, `H`, `L`, vela `i`) y **una zona nueva empieza en `i`**.
    - Si no hay ruptura, la zona se estira: `H` = máx(`H`, máximo de `i`), `L` = mín(`L`, mínimo de `i`), y se suman las velas válidas de `i`.
  - Así, una ruptura alcista necesita que la zona tenga **al menos 3 velas válidas del color contrario** (bajistas) y un **cierre de cuerpo** fuera; una mecha sola nunca rompe.
  - **δ (distancia mínima de la ruptura) = 0**: basta con cerrar estrictamente afuera. Bruni habla de "un mínimo de distancia" sin dar el número (punto abierto 6).
- **Estructura actual** = la dirección de la **última** ruptura registrada hasta `t` inclusive.
- **Estructura periférica** = la dirección de la **penúltima** ruptura.
- **Alineadas** = las dos existen y tienen la misma dirección. Sin dos rupturas desde el último reinicio, no hay estructura y no hay señal.
- **Mínimo de la estructura** (solo H4.3, largos): desde la vela de la última ruptura alcista se busca hacia atrás la **vela bajista válida más cercana**; su **mínimo** es el mínimo de la estructura (entrevista del 02-10-2026). Si no hay ninguna desde el último reinicio, no hay señal. Espejo para cortos: la vela alcista válida más cercana antes de la última ruptura bajista, y su máximo.

## 4. H4.1 Estrategia A literal

**Hipótesis:** dos velas de 15 m seguidas con cuerpo de al menos 15 puntos (60 ticks del NQ) a favor de una estructura alineada, con stop en el extremo de la primera y objetivo de 1,5 R, le agregan retorno ajustado por riesgo a comprar y mantener el US100 después de costos.

**Señal larga** al cierre de la vela `t` (decisión permitida, §2.3):

1. `t − 1` y `t` son **alcistas** y consecutivas.
2. Cuerpo de `t − 1` ≥ **15 puntos** y cuerpo de `t` ≥ **15 puntos**.
3. Estructura actual y periférica **alineadas al alza** al cierre de `t` (§3).

**Operación:**

- **Entrada:** largo en la apertura de `t + 1`.
- **Stop:** el **mínimo de `t − 1`** (el extremo opuesto de la primera vela del impulso).
- **R** = entrada − stop (en puntos). **Objetivo** = entrada + **1,5 × R**.
- Tamaño: el que da el módulo de riesgo para perder 1 % de la cuenta en el stop.

Corto: espejo exacto (velas bajistas, estructura bajista, stop en el máximo de `t − 1`).

**Lo que no se toma del video:** la piramidación, el área de valor como filtro y el bajar a velas de 5 minutos. No se exige que las dos velas sean "válidas" (§3), solo el cuerpo mínimo (punto abierto 7).

**Aviso:** 15 puntos fijos son mucho más grandes, en proporción, cuando el NQ valía 2.800 (2013) que cuando valía 21.000 (2025). Además, con un stop de al menos unos 30 puntos, en los primeros años muchas señales quedarían **a más de 3 ATR** y el riesgo las **rechazaría**. Se espera que H4.1 casi no opere antes de ~2020. Eso es justamente lo que H4.2 corrige. Ver el punto abierto 3 sobre el criterio de las mitades.

## 5. H4.2 Estrategia A normalizada

**Hipótesis:** igual que H4.1, con el cuerpo mínimo medido en volatilidad en vez de en puntos.

**Diferencia con H4.1, solo la condición 2:**

- ATR = ATR de Wilder de 14 velas de 15 m (`cfds_indicators::wilder_atr`, con rango verdadero), calculado sobre **todas** las velas de 15 m, con valor en la vela **`t − 2`** (la anterior a la primera vela del impulso, así ninguna de las dos velas se mide contra un ATR que ya las incluye).
- Cuerpo de `t − 1` ≥ **k × ATR** y cuerpo de `t` ≥ **k × ATR** (el mismo ATR para las dos).
- Calentamiento: sin 14 velas de ATR desde el último reinicio, no hay señal.

Todo lo demás (estructura, entrada, stop, 1,5 R, gestión, riesgo) es idéntico a H4.1.

### 5.1 Cómo se fija k sin mirar resultados (propuesta)

**Propuesta principal: k "equivalente" a los 15 puntos en la época en que Bruni los eligió.**

- **Idea:** él calibró 60 ticks con el NQ de los últimos años. Se busca el k que, en el período más cercano a eso que tenemos permitido, deja pasar **la misma proporción de velas** que el umbral de 15 puntos. Así H4.2 es "la regla de 15 puntos, llevada a otras épocas", y no un número elegido por nosotros.
- **Período de referencia:** **01-11-2024 → 31-10-2025** (los últimos 12 meses antes del fuera de muestra).
- **Velas:** las velas de 15 m de la **sesión de contado** (§2.2) de ese período, con ATR como en §5.
- **Cálculo:**
  1. q = proporción de esas velas con cuerpo ≥ 15 puntos;
  2. k = el cuantil (1 − q) de (cuerpo / ATR) en esas mismas velas;
  3. se redondea al **0,05 más cercano**.
- **Qué mira y qué no:** un script aparte, commiteado antes, que imprime **solo q y k**. No calcula señales, estructuras, operaciones ni resultados. El valor de k se escribe en este archivo **antes de congelar** y la salida del script va en el PR de congelado.
- **Lo que hay que saber:** ese período está dentro de la validación. Mirar la distribución de cuerpos y ATR **no es mirar resultados de la estrategia**, pero es mirar los datos; se declara.

**Alternativa (si el dueño prefiere no mirar nada):** k = **0,5** fijado a priori (un cuerpo de al menos medio ATR, un umbral redondo y habitual para "vela de impulso"). Es más simple y totalmente ciego, pero no está atado a lo que dijo Bruni.

La elección es el punto abierto 2.

## 6. H4.3 Estrategia C ("de campeonato")

Fuente primaria: la transcripción completa con Whisper de la entrevista IQCapital del 02-10-2026 (informe §3.C). Lo que él deja ambiguo está marcado y va a los puntos abiertos.

**Hipótesis:** en una sesión cuyo perfil de volumen se desplaza al alza, comprar cuando el precio vuelve al área de valor de la sesión anterior y la recupera con una vela que cierra sobre el VAH y sobre el máximo de la vela anterior, con volumen "eficiente", con objetivo de 2 R (o el máximo de la estructura, si queda más cerca), le agrega retorno ajustado por riesgo a comprar y mantener el US100 después de costos. Lo mismo, en espejo, para cortos.

### 6.1 Perfil de volumen de la sesión

- **Sesión del perfil:** el **día de operaciones del CME** para el NQ: de las 18:00 ET del día hábil anterior a las 17:00 ET (la sesión que usa por defecto el "Session Volume Profile" de TradingView en un futuro). La sesión `d` es la que termina el día hábil `d`. Alternativa: solo la sesión de contado (punto abierto 9).
- **Barras:** las velas de **1 minuto** del NQ que empiezan dentro de la sesión.
- **Ancho del casillero (bin):** `w` = tick × máx(1, redondeo(0,0002 × apertura de la sesión / tick)), con tick = 0,25 puntos. Es decir, unos 2 puntos básicos del precio redondeados a ticks: unos 4 puntos con el NQ en 20.000 y 0,5 puntos con el NQ en 2.800. Los casilleros arrancan en múltiplos de `w`.
- **Reparto del volumen:** el volumen de cada barra de 1 minuto se reparte **en partes iguales a lo largo de su rango** [mínimo, máximo], en proporción a lo que cada casillero cubre de ese rango. Si mínimo = máximo, todo va al casillero de ese precio.
- **POC** = el casillero con más volumen. Si hay empate, el más cercano al punto medio del rango de la sesión; si sigue el empate, el más bajo.
- **Área de valor (70 %):** empieza en el POC. Mientras el volumen acumulado sea < 70 % del total, se compara el **siguiente casillero de arriba** con el **siguiente de abajo** y se suma el de más volumen; si empatan, se suman los dos; si ya no queda casillero de un lado, se suma el del otro.
- **VAH** = borde superior del casillero más alto del área; **VAL** = borde inferior del casillero más bajo.
- **Perfil cerrado** de la sesión `d − 1`: se usa recién después de que esa sesión termina.
- **Perfil en desarrollo** de la sesión `d` hasta la vela `t`: el mismo cálculo con las barras de 1 minuto de `d` que empiezan antes del cierre de `t`. No usa nada posterior a `t`.
- Si a la sesión `d − 1` le falta más del 10 % de sus barras de 1 minuto, su perfil no existe y no hay señales en `d`.

### 6.2 Señal larga al cierre de la vela `t`

Sean `d` la sesión de `t` y `d − 1` la anterior.

1. **Tendencia (desplazamiento del perfil):** el POC en desarrollo de `d` hasta `t` > POC(`d − 1`) **y** el VAL en desarrollo de `d` > VAL(`d − 1`). (Él dice que el área de valor y el POC de la sesión actual quedan "por encima" de los de la anterior.)
2. **Estructura:** la estructura actual es alcista al cierre de `t` (§3).
3. **Vuelta al área de valor:** alguna vela de la sesión `d`, desde su inicio hasta `t − 1`, tuvo mínimo ≤ VAH(`d − 1`). Alcanza con tocarla.
4. **Sin perforar la estructura:** desde la última ruptura alcista hasta `t`, ningún mínimo quedó por debajo del **mínimo de la estructura** (§3). Si lo perforó, no hay señales hasta una ruptura alcista nueva.
5. **Disparador**, todo en la vela `t`:
   - a. cierre de `t` ≥ máximo de `t − 1` + **m** ("cuerpo que cierra sobre la mecha más alta de la anterior");
   - b. cierre de `t` ≥ VAH(`d − 1`) + **m**;
   - c. **volumen eficiente:** volumen de `t` < volumen de `t − 1` **o** volumen de `t` < promedio simple del volumen de las 20 velas `t − 20` a `t − 1`. El volumen de una vela de 15 m es la suma del de sus 15 barras de 1 minuto.
   - **m = 6 ticks = 1,5 puntos** (la lectura más probable de su "6 para el Nasdaq", medido en ticks como el resto de sus medidas; punto abierto 10).
6. **Primera operación de la secuencia:** la primera operación después de cada vuelta al área de valor (condición 3) tiene que tener cierre de `t − 1` < VAH(`d − 1`) + m, o sea que `t` es la vela que rompe el VAH. Las **reentradas** (§6.3) no lo necesitan.
7. **Sin datos fuertes:** no hay comunicado programado del FOMC (`official_releases`, `fed_fomc`, como en el programa Nasdaq §2.3) ni de **CPI** o **empleo** del BLS con `scheduled_at` dentro de la sesión de contado de `d`. Él dice "durante el disparador" y "días con muchas noticias"; se toma el día entero porque es más simple y más conservador (punto abierto 11). Hoy el BLS no está cargado (ADR 0016): H4.3 **no se corre** hasta que esté, igual que H2 del programa Nasdaq.
8. **Hay recorrido:** el **máximo de la estructura** es mayor que el precio de entrada. Si no, no se opera.

**Máximo de la estructura** (largos) = el mayor máximo entre la vela de la última ruptura alcista y la última vela anterior a la primera que tocó el área de valor (condición 3). Es "el máximo anterior" al que él apunta.

### 6.3 Operación

- **Entrada:** largo en la apertura de `t + 1`. Él entra al cierre de `t`; acá es la apertura de la vela siguiente (`CLAUDE.md` §5b).
- **Stop:** el **mínimo de `t`**.
- **R** = entrada − stop. **Objetivo** = el menor entre entrada + **2 × R** y el **máximo de la estructura**.
- **Reentradas:** después de cerrar una operación de la secuencia, se puede volver a entrar con las condiciones 1, 2, 4, 5, 7 y 8, hasta que el precio toque el máximo de la estructura. Ahí la secuencia termina: no hay más operaciones hasta una nueva vuelta al área de valor.

Corto: espejo exacto (POC y VAH en desarrollo por debajo de los de `d − 1`, estructura bajista, vuelta con máximo ≥ VAL(`d − 1`), cierre ≤ mínimo de `t − 1` − m y ≤ VAL(`d − 1`) − m, stop en el máximo de `t`, piso de la estructura como límite del objetivo).

### 6.4 Régimen, frescura y autocorrelación: no son filtros

Según la entrevista, el régimen (percentil del ATR), la frescura, la posición en la racha, el tipo de volumen y la sesión **solo suman puntos para el tamaño** ("meta-label" y Kelly). Él toma la operación igual, aunque sume 1 de 5. Como acá el riesgo es fijo (1 %), **H4.3 no filtra por ninguno de ellos.** El régimen se calcula solo como **descriptivo**:

- ATR(14) de Wilder de 15 m sobre todas las velas (§5);
- **percentil** en la vela `t` = 100 × (cantidad de valores de ATR entre `t − 199` y `t` que son ≤ ATR de `t`) / 200;
- tramos: **bajo** ≤ 25; **medio** > 25 y ≤ 50; **alto** > 50 y ≤ 75; **extremo** > 75;
- se informa el resultado de H4.3 por tramo, sin voto.

**Lo que no se toma de la entrevista:** la frescura bayesiana (no es reproducible), el meta-label y el Kelly (§9).

## 7. Criterios para pasar (iguales para H4.1, H4.2 y H4.3)

Series, igual que el programa Nasdaq §3.3:

- `y` = retornos mensuales netos de la hipótesis (153 meses; un mes sin operaciones da 0), cada escenario de financiación.
- `x` = retornos mensuales de comprar y mantener US100, mismo escenario.
- `y_s`, `x_s` = las mismas con todos los costos × 1,5.

Criterios (todos, en los **dos** escenarios EUR y USD):

1. **Alfa contra comprar y mantener:** `y = α + β x + e` por mínimos cuadrados, error de `α` robusto (White, HC1), prueba t unilateral (H0: α ≤ 0) con `n − 2` grados de libertad (`research::stats::alpha_hc1`). Pasa si **p < 0,0125**.
2. **Sharpe deflactado de la ventaja** `y_t − β x_t` (`research::stats::deflated_sharpe`), con el **N del registro al congelar** (§0). Pasa si **DSR ≥ 0,95**. Si la fórmula no da valor, no pasa.
3. **Mitades:** la media de `y_t − β x_t` es **> 0** en **02-2013 → 12-2020** y en **01-2021 → 10-2025**.
4. **Costos × 1,5:** el alfa con `y_s` y `x_s` sigue **> 0**.
5. **Agrega a comprar y mantener:** la cartera mensual `0,5 x_t + 0,5 y_t` tiene un **Sharpe mensual mayor** que `x` solo.

**Una hipótesis pasa solo si cumple los 5 criterios en los dos escenarios.** Además, **mínimo de muestra:** si una hipótesis tiene menos de **100 operaciones** en todo el período, no pasa aunque cumpla lo demás (es el mínimo que el propio Bruni pide; punto abierto 4).

### 7.1 Descriptivo (sin voto)

- Por hipótesis: operaciones (largas y cortas), salidas por motivo (stop, objetivo, cierre del día, vencimiento), rechazos del riesgo por motivo, R medio antes y después de costos, costo medio en R, aciertos, operaciones por año, retorno mensual medio y Sharpe, DSR con N = 100 y N = 1000, resultado con costos × 2.
- Las velas que tocan stop y objetivo a la vez, resueltas con barras de 1 minuto (§2.4).
- Operaciones que no se habrían abierto con 10.000 €.
- Para H4.3: cuántas señales quedan después de cada condición (desplazamiento, vuelta al área, cierre sobre el máximo anterior, cierre sobre el VAH, volumen, datos fuertes, recorrido), cuántas son primeras operaciones y cuántas reentradas, cuántos objetivos quedaron recortados por el máximo de la estructura, y el resultado por tramo de régimen (§6.4).
- **Autocorrelación de resultados** de cada hipótesis: tasa de aciertos después de una ganadora y después de una perdedora, contra la general, en ventanas móviles de 100 operaciones.
- **Test de suerte** (§7.2).

### 7.2 Test de suerte (solo reporte)

Para cada hipótesis, **1.000 simulaciones** con semilla fija (**20261125**; el generador pseudoaleatorio determinístico concreto se nombra al congelar, porque hoy el motor no tiene ninguno):

- cada simulación tiene **la misma cantidad de operaciones por sesión** que la estrategia real;
- cada operación al azar entra en una vela de ejecución sorteada (uniforme) entre las permitidas de esa sesión (§2.3), sin superponerse con otra operación de la misma simulación;
- dirección: larga o corta con la **misma proporción** de largos que la estrategia;
- **misma distancia de stop en puntos** que la operación real con la que se empareja (en orden) y el **mismo objetivo en R** (1,5 o 2);
- mismos costos, misma gestión (§2.4) y mismo riesgo.

Se informa en qué percentil de las 1.000 simulaciones cae el R medio y el retorno total de la estrategia. **No vota**: es una forma fácil de leer el resultado.

## 8. Reserva (1,25 %): candidata "saltear tras una pérdida"

**No se asigna en este borrador** salvo que el dueño lo decida antes de congelar (punto abierto 5). Si se asigna, la definición es esta y no cambia:

- **Base:** H4.3 (la estrategia en la que Bruni dice usar este filtro). En la entrevista del 02-10-2026 lo describe más fino (la mejor operación es la que sigue a la primera ganadora después de pérdidas); acá se prueba la versión simple de su canal, que tiene menos grados de libertad. Elegirla ahora, antes de cualquier resultado, evita elegir después la estrategia que mejor dio.
- **Regla causal por ventana:** para cada ventana de validación, se corre H4.3 en sus **24 meses de entrenamiento** y se mide:
  - `a` = tasa de aciertos después de una pérdida; `g` = tasa general;
  - estabilidad: en ventanas móviles de 100 operaciones dentro del entrenamiento, la proporción de ventanas con `a < g`.
- **Filtro activo en la validación** de esa ventana solo si `a < g` con una prueba z unilateral de dos proporciones con p < 0,05 **y** al menos 2/3 de las ventanas móviles con `a < g`. Activo = **no se toma la operación que sigue a una pérdida** (la siguiente sí).
- Criterios: los mismos 5 de §7 (contra comprar y mantener, no contra H4.3), con p < 0,0125. Se informa también la diferencia contra H4.3 sin filtro, sin voto.

Si el dueño no la asigna ahora, la reserva queda sin gastar y solo se usa con un pre-registro nuevo y fechado.

## 9. Fuera de alcance

- **Piramidación** (sumar tamaño y mover el stop): cambia el riesgo después de entrar y no tiene una definición numérica en las fuentes.
- **Riesgo variable:** el VAM (0,5 % a 4 % por operación), el Kelly fraccional y el meta-tagging. Chocan con `config/risk.toml` (1 % por operación, 3 % diario) y la spec. No se prueban en este programa.
- Las velas de 5 minutos, el filtro de distancia entre mechas, la "frescura" bayesiana, invertir la estrategia en régimen calmo, otros instrumentos.
- **Pasar a real:** aunque una hipótesis pase, operarla pide un cambio de spec aprobado aparte, y el dinero real necesita un ADR de dos personas (`CLAUDE.md` §5).

## 10. Prohibido después de ver resultados

Todo esto cuenta como una prueba nueva, con su costo en el presupuesto:

- cambiar el umbral (15 puntos, k, m), el objetivo (1,5 R, 2 R), el stop, el marco de tiempo, el horario, el cierre del día o el tamaño;
- cambiar las definiciones de vela válida, zona, ruptura, δ, actual y periférica, mínimo y máximo de la estructura, perfil de volumen (sesión, casillero, reparto, 70 %), régimen (14/200, tramos), eficiencia de volumen (media de 20) o el margen m;
- recalcular k con otro período o redondeo;
- agregar o sacar filtros (régimen, frescura, días de datos, periférica en H4.3), filtrar por año, por sesión o por dirección (los cortes del §7.1 son descriptivos);
- asignar la reserva a la hipótesis que mejor dio;
- cambiar los costos, el escenario de financiación (los dos votan), el estrés, la vara de comparación, el corte de las mitades, el nivel 0,0125, el mínimo 0,95, el N o el mínimo de 100 operaciones;
- mirar el fuera de muestra sin la aprobación del dueño;
- volver a correr con otra regla y presentar la mejor.

**Errores de código:** si después de ver los resultados aparece un error de código (no de la regla), se corrige, se vuelve a correr y se informa con fecha: cuál fue el error, el resultado anterior y el nuevo. El anterior no se borra (fila propia en el registro, con `compara_con`).

## 11. Al congelar y al correr

- **Congelar:** commitear este archivo sin "borrador", con k ya escrito (§5.1), su `.sha256` y la salida del script de k. En el mismo PR, una sección nueva **`[rule_program4]`** en `config/research.toml` (`alpha = "0.0125"`, `dsr_trials` = el N al congelar, `dsr_min = "0.95"`, `cost_stress = "1.5"`, `second_half_from = "2021-01-01"`) y los parámetros de las reglas con los valores exactos de este archivo. Las secciones anteriores no cambian. `config/strategy.toml`, `config/risk.toml`, el runner y el paper no cambian.
- **Código:** las definiciones de §3 y §6 son nuevas. `engine/crates/patterns` ya tiene un "engulfing" de cuerpo contra cuerpo; **no es el de Bruni** (cierre sobre el máximo de la vela anterior). Las funciones nuevas van en Rust, como funciones puras con tests (incluidos los de no repintado), y se registran en `docs/ARCHITECTURE.md` en su PR.
- **Correr:** cada corrida se anota con su hora (UTC) y su comando, y agrega su fila en `docs/analysis/registro-de-resultados.csv` en el mismo PR que su informe. Antes de calcular retornos, la corrida imprime cuántas señales da cada hipótesis por año.
- **Si pasa:** se informa al dueño; él decide si se mira el fuera de muestra (una vez, con la regla congelada) y si se abre un Issue de spec. Pasar no habilita operar.
- **Si no pasa:** la hipótesis se cierra. No se prueban variantes sobre estos datos.

## 12. Puntos abiertos antes de congelar

**Cerrados el 25-11-2026: ver §13.** Se dejan como estaban para que se vea qué se preguntó.

Los decide el dueño. Cada respuesta queda en #561 con sus palabras y la fecha.

1. **Presupuesto total del proyecto:** ¿el 5 % del programa 4 convive con el 5 % que pide el programa 3 (#513), o uno reemplaza al otro? Si conviven, la probabilidad de algún falso positivo en el proyecto sube.
2. **k de H4.2:** ¿la calibración por proporción equivalente en 11-2024 → 10-2025 (§5.1, mira la distribución de velas pero no resultados) o k = 0,5 fijado a ciegas?
3. **Criterio de las mitades en H4.1:** con 15 puntos fijos, H4.1 casi no opera antes de ~2020 (§4), y la primera mitad tendrá una media cercana a 0 que puede caer de cualquier lado. ¿Se deja el criterio igual (y H4.1 queda como "la regla literal tal cual", probablemente sin pasar) o se cambia algo **ahora**, antes de ver resultados?
4. **Mínimo de 100 operaciones** (§7): ¿se agrega como condición para pasar?
5. **Reserva:** ¿se asigna ahora a "saltear tras una pérdida" sobre H4.3 (§8), o queda sin gastar?
6. **δ de la ruptura:** ¿0 (cerrar estrictamente afuera) o un mínimo en ATR? Ni el video del 30-10 ni la transcripción completa de la entrevista del 02-10-2026 dan el número.
7. **H4.1 y H4.2:** ¿se exige además que las dos velas del impulso sean "válidas" (cuerpo mayor que la mecha más larga)? En el video solo habla del tamaño del cuerpo.
8. **H4.3, estructura:** ¿se exige también la estructura periférica alineada, o solo la actual (§6.2, condición 2)? En la entrevista solo usa el mínimo de la estructura.
9. **H4.3, sesión del perfil:** ¿el día del CME (18:00–17:00 ET, lo que usa TradingView por defecto en un futuro; propuesta) o solo la sesión de contado?
10. **H4.3, margen m:** dice "6 para el Nasdaq" medido con la regla de TradingView. ¿**6 ticks = 1,5 puntos** (propuesta, porque mide las velas en ticks) o 6 puntos? Con cualquiera de los dos, el margen pesa mucho más en 2013 (NQ ≈ 2.800) que en 2025: ¿se deja fijo (literal) o se escala con el precio?
11. **H4.3, datos fuertes:** ¿se excluye el día entero de FOMC, CPI y empleo (propuesta) o solo si el dato cae en la vela del disparador? ¿Se espera la carga del BLS o se corre filtrando solo el FOMC?
12. **H4.3, media del volumen:** él habla de "MA 20" y también de "EMA". ¿Media simple de 20 (propuesta) o exponencial de 20?
13. **H4.3, tendencia:** ¿perfil en desarrollo de la sesión actual contra el de la anterior (propuesta, como en su ejemplo) o sesión anterior contra la de antes? ¿Se exigen POC y VAL más altos (propuesta) o alcanza con el POC?
14. **Conteo de señales antes de congelar:** ¿se autoriza correr **solo el conteo** de señales por año de cada hipótesis (sin P/L ni resultados) para ver si hay muestra suficiente? Si una hipótesis da muy pocas señales, conviene saberlo antes de gastar presupuesto en ella.
15. **Vencimientos y volumen:** `NQ.c.0` es el contrato más cercano en el calendario; en la semana del vencimiento el volumen se pasa al contrato siguiente y el perfil del NQ se deforma. ¿Se excluyen las sesiones desde la fecha de cambio de contrato habitual del CME (8 días antes del vencimiento) hasta el vencimiento, o se acepta el efecto?
16. **Congelar:** commitear con fecha, hora y SHA-256, escribir k, agregar `[rule_program4]` y sumar las pruebas al registro.

## 13. Decisiones del dueño

**Aprobación del dueño** (Alexei), 25-11-2026, registrada en #561, textual: **"apruebo las 15"**. Aprueba las recomendaciones de los puntos abiertos 1 a 15 de §12. El punto 16 es el procedimiento de congelado (§11). Cuando una decisión cambia algo escrito más arriba, **manda esta sección**.

| Punto | Decisión |
|---|---|
| 1. Presupuesto | Los programas 3 y 4 tienen **presupuestos separados**: el programa 4 usa su 5 % propio (§0) y el programa 3 (#513) pide el suyo aparte. |
| 2. k de H4.2 | **Calibración por proporción** de velas en 01-11-2024 → 31-10-2025 (§5.1, propuesta principal), sin mirar resultados. La alternativa k = 0,5 queda descartada. |
| 3. Mitades en H4.1 | **H4.1 literal, sin cambios**: el criterio de las mitades queda igual que en §7. |
| 4. Mínimo de muestra | **Mínimo de 100 operaciones** en todo el período: con menos, la hipótesis no pasa (§7). |
| 5. Reserva | **Guardada**, sin asignar. Solo se usa con un pre-registro nuevo y fechado (§8). |
| 6. δ de la ruptura | **δ = 0**: alcanza con cerrar estrictamente afuera, sin distancia extra (§3). |
| 7. Velas del impulso | En H4.1 y H4.2 las dos velas del impulso (`t − 1` y `t`) tienen que ser **válidas** (§3: cuerpo > mecha más larga), además de cumplir el cuerpo mínimo. Reemplaza la última frase de "Lo que no se toma del video" en §4. |
| 8. Periférica en H4.3 | **H4.3 sin periférica**: solo la estructura actual (§6.2, condición 2, como estaba). |
| 9. Sesión del perfil | **Día del CME completo** (18:00–17:00 ET, §6.1). |
| 10. Margen m | **6 ticks = 1,5 puntos fijos**, sin escalar con el precio. |
| 11. Datos fuertes | **Se saltea el día entero** con un dato fuerte (FOMC, CPI o empleo del BLS con hora programada dentro de la sesión de contado). **H4.3 espera** la carga del calendario del BLS. |
| 12. Media del volumen | **SMA 20** (media simple de 20 velas, §6.2). |
| 13. Tendencia en H4.3 | **Sesión actual (perfil en desarrollo) contra la anterior**, con POC **y** VAL más altos (§6.2, condición 1). |
| 14. Conteo de señales | **Autorizado**: antes de congelar se cuentan las señales por año de cada hipótesis, **sin P/L ni ningún resultado**. Si una hipótesis da **menos de 100 señales** en total en el walk-forward, no se congela: se frena y se le informa al dueño. |
| 15. Vencimientos | **Se excluye solo el día de roll** (no la semana del vencimiento), además del reinicio de §2.5. |

### 13.1 Alcance de las decisiones 11 y 15 en H4.1 y H4.2

- **Decisión 11 (datos fuertes):** se aplica también a H4.1 y H4.2. Así lo leyó la sesión coordinadora (Claude Code) del dueño el 25-11-2026 al pasar la aprobación ("se saltea el día entero con datos fuertes y C espera el calendario del BLS": el día se saltea en todas, y solo C espera). Como hoy `official_releases` tiene **solo el FOMC** (el BLS no está cargado, ADR 0016), **H4.1 y H4.2 se corren salteando solo los días con comunicado programado del FOMC** (`fed_fomc`, `fomc_statement`, estado `scheduled`, con `scheduled_at` dentro de la sesión de contado de ese día). Los días de CPI y empleo **no** se excluyen en H4.1 y H4.2: es una limitación declarada, no una elección después de mirar.
- **Decisión 15 (día de roll):** se aplica a las tres hipótesis. El **día de roll** es la primera sesión de contado (XNYS) que abre en o después del primer instante del contrato nuevo, según el calendario de vencimientos de la preparación común (`[futures.roll]`, `after_expiry_day` para el NQ). Ese día no hay señales (ninguna vela de ejecución cae en él).

### 13.2 Precisiones de implementación fijadas al congelar

Se escriben **antes** de correr nada y no cambian ninguna regla de arriba; solo dejan escrito lo que la implementación tiene que hacer donde el texto no lo dice.

- **Velas de 15 m:** las de `futures_candles` (NQ.c.0), que ya arma `cfds_market_data::futures_candles` desde `futures_bars_1m` (#259: ventana [k·15 min, (k+1)·15 min) UTC, solo velas cerradas y de un solo contrato). No se arma otra agregación.
- **ATR:** el de Wilder de 14 (`wilder_atr`) calculado **por tramo de contrato** (se reinicia en cada vencimiento, §2.5). El mismo ATR es el del módulo de riesgo en la vela de la decisión `t`.
- **Estructura:** las rupturas de §3 se calculan vela a vela desde el último reinicio; la estructura en `t` incluye una ruptura en la propia vela `t` (se conoce al cierre de `t`).
- **Vela de ejecución** (`t + 1`): empieza en o después de la apertura de la sesión de contado de su día y **antes** del cierre menos 30 minutos; su día no es día de roll ni día excluido por datos fuertes. **Salida planificada:** al cierre de la última vela que termina en o antes del cierre menos 15 minutos (la apertura de la última vela de la sesión); si esa vela no es posterior a `t`, no hay señal.
- **Objetivo:** con el precio medio de la entrada `E` y el stop `S`, R = |E − S| y el objetivo es E ± 1,5 R. Si la apertura de una vela ya pasó el objetivo, sale al objetivo (no a la apertura). Si en la apertura de `t + 1` el precio ya quedó del lado del stop, no se opera (§2.4) y se cuenta aparte.
- **Calibración de k (§5.1):** velas de la sesión de contado (empiezan en o después de la apertura y terminan en o antes del cierre) que empiezan en 01-11-2024 → 31-10-2025; para cada vela, cuerpo / ATR de la vela **anterior** (contigua, del mismo tramo de contrato; es el ATR que §5 usa para la primera vela del impulso). q = proporción con cuerpo ≥ 15 puntos; k = el cuantil (1 − q) de los cocientes con la definición de la inversa de la distribución empírica (el valor ordenado número ⌈(1 − q)·n⌉, mínimo 1); redondeo al múltiplo de 0,05 más cercano (las mitades, hacia arriba).
- **Señales para el conteo (decisión 14):** cada vela `t` que cumple todas las condiciones de la señal, con la decisión dentro de la validación concatenada (01-02-2013 → 01-11-2025), antes de la regla de una posición a la vez y antes del módulo de riesgo.
- **Test de suerte (§7.2):** generador **SplitMix64** (Steele, Lea y Flood, 2014) con semilla 20261125, un solo flujo para las 1.000 simulaciones en orden. Índice uniforme en [0, n) = (x · n) >> 64 con x de 64 bits; número en [0, 1) = (x >> 11) / 2⁵³. Por simulación y por sesión con `n` operaciones reales: se sortean `n` velas de ejecución distintas entre las permitidas de esa sesión (las mismas condiciones de la vela de ejecución de arriba, con la decisión permitida en la vela anterior), se ordenan y se simulan en orden; una vela que cae antes de la salida de la operación anterior de la misma simulación se descarta (esa simulación tiene una operación menos). Cada operación sorteada usa, en orden, la distancia de stop en puntos (|E − S|) de la operación real con que se empareja, la dirección larga si el número en [0, 1) es menor que la proporción de largos de la estrategia, 1,5 R, la gestión de §2.4 (`simulate_leg`) y los costos de `fill_price`. Se compara el **R neto medio** y el **retorno total** con 1 % de riesgo por operación (∏(1 + 0,01 · R neto) − 1) de la estrategia, medidos igual, contra los de las simulaciones (percentil = porcentaje de simulaciones con un valor menor). No vota.
