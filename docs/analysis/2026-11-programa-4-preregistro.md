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
| H4.3 | Estrategia C ("de campeonato"): perfil de volumen de sesión, régimen, cierre sobre el VAH, 1:2 | 1,25 % (p < 0,0125) |
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
- **Mínimo de la estructura** (solo H4.3, largos): el mínimo de la **última vela válida anterior** a la vela de la última ruptura alcista. Espejo para cortos (máximo de la última vela válida anterior a la última ruptura bajista).

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

**Hipótesis:** comprar (vender) cuando el precio vuelve al área de valor de la sesión anterior y la recupera con una vela que cierra sobre el VAH (bajo el VAL) y sobre el máximo (bajo el mínimo) de la vela anterior, con volumen "eficiente", en régimen de volatilidad alto y a favor del desplazamiento del área de valor, con objetivo de 2 R, le agrega retorno ajustado por riesgo a comprar y mantener el US100 después de costos.

### 6.1 Perfil de volumen de la sesión

Para cada sesión de contado `d`:

- **Barras:** las velas de **1 minuto** del NQ que empiezan dentro de la sesión de contado de `d` (09:30 a 15:59 ET un día normal).
- **Ancho del casillero (bin):** `w` = tick × máx(1, redondeo(0,0002 × apertura de la sesión / tick)), con tick = 0,25 puntos. Es decir, unos 2 puntos básicos del precio redondeados a ticks: unos 4 puntos con el NQ en 20.000 y 0,5 puntos con el NQ en 2.800. Los casilleros arrancan en múltiplos de `w`.
- **Reparto del volumen:** el volumen de cada barra de 1 minuto se reparte **en partes iguales a lo largo de su rango** [mínimo, máximo], en proporción a lo que cada casillero cubre de ese rango. Si mínimo = máximo, todo va al casillero de ese precio.
- **POC** = el casillero con más volumen. Si hay empate, el más cercano al punto medio del rango de la sesión; si sigue el empate, el más bajo.
- **Área de valor (70 %):** empieza en el POC. Mientras el volumen acumulado sea < 70 % del total, se compara el **siguiente casillero de arriba** con el **siguiente de abajo** y se suma el de más volumen; si empatan, se suman los dos; si ya no queda casillero de un lado, se suma el del otro.
- **VAH** = borde superior del casillero más alto del área; **VAL** = borde inferior del casillero más bajo.
- El perfil de `d` existe recién **después del cierre de la sesión `d`**: solo se usa en sesiones posteriores.
- Si a la sesión le falta más del 10 % de sus barras de 1 minuto, su perfil no existe y no hay señales que dependan de él.

### 6.2 Régimen de volatilidad (percentil del ATR 14/200)

- ATR(14) de Wilder de 15 m sobre todas las velas (§5).
- **Percentil** en la vela `t` = 100 × (cantidad de valores de ATR entre `t − 199` y `t` que son ≤ ATR de `t`) / 200.
- Régimen: **bajo** ≤ 25; **medio** > 25 y ≤ 50; **alto** > 50 y ≤ 75; **extremo** > 75.
- Calentamiento: 14 + 199 velas desde el último reinicio.
- **Filtro de H4.3: solo régimen alto** (el que Bruni dice que mejor rinde; punto abierto 9).

### 6.3 Señal larga al cierre de la vela `t`

Sean `d` la sesión de `t`, `d − 1` la sesión de contado anterior y `d − 2` la de antes, con perfiles VAH/VAL según §6.1.

1. **Tendencia por desplazamiento del área de valor:** VAL(`d − 1`) > VAL(`d − 2`) **y** VAH(`d − 1`) > VAH(`d − 2`).
2. **Estructura:** la estructura actual es alcista al cierre de `t` (§3). (No se exige la periférica: Bruni no la menciona en esta estrategia; punto abierto 8.)
3. **Vuelta al área de valor:** alguna vela de la sesión `d`, desde su apertura hasta `t − 1`, tuvo mínimo ≤ VAH(`d − 1`); y el cierre de `t − 1` ≤ VAH(`d − 1`).
4. **Sin romper la estructura:** ninguna vela desde la última ruptura alcista hasta `t` cerró por debajo del **mínimo de la estructura** (§3).
5. **Vela de entrada ("envolvente" de Bruni):** cierre de `t` > VAH(`d − 1`) **y** cierre de `t` ≥ máximo de `t − 1` + **m**, con **m = 0,4 × k × ATR** (ATR en `t − 1`). El 0,4 es la proporción entre los 6 puntos que dice Bruni para esta regla y los 15 de la estrategia A; así m queda atado al mismo k de H4.2 y escala con la volatilidad (punto abierto 10: o 6 puntos fijos).
6. **Eficiencia de volumen:** volumen de `t` < volumen de `t − 1` **o** volumen de `t` < promedio del volumen de las 20 velas `t − 20` a `t − 1`. Volumen de una vela de 15 m = suma del de sus 15 barras de 1 minuto.
7. **Régimen alto** en `t` (§6.2).
8. **Sin datos fuertes:** la sesión `d` no es día de comunicado programado del FOMC (`official_releases`, `fed_fomc`, como en el programa Nasdaq §2.3) ni de **CPI** o **empleo** del BLS. Hoy el BLS no está cargado (ADR 0016): H4.3 **no se corre** hasta que esté, igual que H2 del programa Nasdaq (punto abierto 11).

**Operación:**

- **Entrada:** largo en la apertura de `t + 1`.
- **Stop:** el **mínimo de `t`** (la vela de entrada).
- **R** = entrada − stop. **Objetivo** = entrada + **2 × R**.
- **Reentradas:** después de cerrar una operación, se puede volver a entrar en la misma sesión si se cumplen de nuevo todas las condiciones (con una vela `t` posterior a la salida).

Corto: espejo exacto (área de valor desplazada a la baja, estructura bajista, vuelta al área con máximo ≥ VAL y cierre de `t − 1` ≥ VAL, cierre de `t` < VAL y ≤ mínimo de `t − 1` − m, stop en el máximo de `t`).

**Lo que no se toma de la entrevista:** la "frescura" bayesiana (no es reproducible), el cierre total en el "máximo de la estructura" (no está definido con precisión), el meta-tagging y el Kelly (§9).

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
- Para H4.3: cuántas señales quedan después de cada filtro (desplazamiento, vuelta al área, envolvente, volumen, régimen, datos fuertes). Y la misma regla **sin** el filtro de régimen, solo como contexto.
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

- **Base:** H4.3 (la estrategia en la que Bruni dice usar este filtro). Elegirla ahora, antes de cualquier resultado, evita elegir después la estrategia que mejor dio.
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
- cambiar las definiciones de vela válida, zona, ruptura, δ, actual y periférica, mínimo de la estructura, perfil de volumen (casillero, reparto, 70 %), régimen (14/200, tramos) o eficiencia de volumen;
- recalcular k con otro período o redondeo;
- agregar o sacar filtros (régimen, días de datos, periférica en H4.3), filtrar por año, por sesión o por dirección (los cortes del §7.1 son descriptivos);
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

Los decide el dueño. Cada respuesta queda en #561 con sus palabras y la fecha.

1. **Presupuesto total del proyecto:** ¿el 5 % del programa 4 convive con el 5 % que pide el programa 3 (#513), o uno reemplaza al otro? Si conviven, la probabilidad de algún falso positivo en el proyecto sube.
2. **k de H4.2:** ¿la calibración por proporción equivalente en 11-2024 → 10-2025 (§5.1, mira la distribución de velas pero no resultados) o k = 0,5 fijado a ciegas?
3. **Criterio de las mitades en H4.1:** con 15 puntos fijos, H4.1 casi no opera antes de ~2020 (§4), y la primera mitad tendrá una media cercana a 0 que puede caer de cualquier lado. ¿Se deja el criterio igual (y H4.1 queda como "la regla literal tal cual", probablemente sin pasar) o se cambia algo **ahora**, antes de ver resultados?
4. **Mínimo de 100 operaciones** (§7): ¿se agrega como condición para pasar?
5. **Reserva:** ¿se asigna ahora a "saltear tras una pérdida" sobre H4.3 (§8), o queda sin gastar?
6. **δ de la ruptura:** ¿0 (cerrar estrictamente afuera) o un mínimo en ATR? La transcripción completa de la entrevista del 02-10-2026 puede dar el número.
7. **H4.1 y H4.2:** ¿se exige además que las dos velas del impulso sean "válidas" (cuerpo mayor que la mecha más larga)? En el video solo habla del tamaño del cuerpo.
8. **H4.3:** ¿se exige también la estructura periférica alineada, o solo la actual (§6.3, condición 2)?
9. **Régimen en H4.3:** ¿solo "alto" (lo que dice Bruni), "alto o extremo", o sin filtro? Con solo "alto" puede haber pocas operaciones.
10. **Margen m de la envolvente en H4.3:** ¿0,4 × k × ATR (escalado) o 6 puntos fijos (literal)?
11. **Datos fuertes en H4.3:** ¿se espera la carga del BLS para filtrar CPI y empleo, o se corre filtrando solo el FOMC?
12. **Conteo de señales antes de congelar:** ¿se autoriza correr **solo el conteo** de señales por año de cada hipótesis (sin P/L ni resultados) para ver si hay muestra suficiente? Si una hipótesis da muy pocas señales, conviene saberlo antes de gastar presupuesto en ella.
13. **Vencimientos y volumen:** `NQ.c.0` es el contrato más cercano en el calendario; en la semana del vencimiento el volumen se pasa al contrato siguiente y el perfil del NQ se deforma. ¿Se excluyen las sesiones desde la fecha de cambio de contrato habitual del CME (8 días antes del vencimiento) hasta el vencimiento, o se acepta el efecto?
14. **Confirmar con la transcripción completa** de la entrevista IQCapital del 02-10-2026 (la que hay tiene la pista doblada): el margen de 6 puntos, la definición del volumen "eficiente", la sesión del perfil (contado de Nueva York o el día entero), el ATR 14/200 y el 1:2.
15. **Congelar:** commitear con fecha, hora y SHA-256, escribir k, agregar `[rule_program4]` y sumar las pruebas al registro.
