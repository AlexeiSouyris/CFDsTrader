# Fase 1 — Especificación v1 (para aprobar)

- **Estado:** propuesta, pendiente de aprobación de los dos colaboradores.
- **Fuente:** respuestas en [`fase-1-preguntas.md`](fase-1-preguntas.md). Los términos enlazan al [Glosario](../GLOSARIO.md).
- **Regla:** todo número de este documento es un **valor inicial de configuración** (`config/`), no un valor fijo en el código. Cambiarlo no requiere tocar código, pero sí dejarlo registrado.
- **Objetivo:** dos personas que lean esto harían exactamente las mismas operaciones.

---

## 1. Rol del sistema

1. El sistema **analiza y propone** operaciones. **No envía órdenes reales.**
2. Cada propuesta llega a 1 o 2 **responsables** designados, que la aprueban y la ejecutan a mano en el broker.
3. En paralelo, el sistema lleva un **P&L simulado** ([paper trading](../GLOSARIO.md#paper-trading)), como si hubiera ejecutado todo lo que propuso. Así se mide el sistema y no la disponibilidad de los responsables.
4. También se registra lo que los responsables ejecutaron de verdad, para comparar.
5. **Responsables:** los dos colaboradores. Cualquiera de los dos puede aprobar una propuesta.
6. **Broker para datos en tiempo real, costos y demo:** IG, con cuenta demo a nombre del titular. Se confirma en un ADR en la fase 2.

### 1.1 App en el celular, cuentas y seguridad
- Las propuestas, las alertas (noticias en contra, kill switch, errores) y las votaciones llegan como **notificaciones al iPhone** de cada miembro, y se responden desde ahí.
- **Acceso privado:** solo los 6 miembros pueden instalar la app y entrar. No es pública.
- **Cuenta individual por miembro**, con **2FA obligatorio** mediante una app autenticadora (Google Authenticator o similar, TOTP).
- **Roles:**
  - *Responsable:* aprueba o rechaza propuestas.
  - *Miembro:* ve su P&L y el del fondo, recibe alertas y vota.
- Toda acción (aprobar, votar, aportar, retirar) queda registrada con usuario y hora (UTC).
- **Distribución:** app web instalable (PWA). Se abre en Safari, se agrega a la pantalla de inicio y recibe notificaciones push. No pasa por la App Store. Si más adelante hace falta, se evalúa una app nativa.

### 1.2 Votaciones (desde la app, 1 voto por miembro)
| Qué se vota | Mayoría necesaria | Plazo |
|---|---|---|
| Cambios de reglas o parámetros (riesgo, instrumentos, apalancamiento) | 4 de 6 | 48 h |
| Reactivar el sistema después del kill switch | 4 de 6 **y** los 2 responsables a favor | sin plazo |
| Operación grande (nocional > 5 × capital del fondo) | 4 de 6, dentro de la validez de la propuesta (10 min) | 10 min; si no se llega, no se ejecuta |
| Pasar a dinero real, y con cuánto | **6 de 6** (unanimidad) | 7 días |

Los umbrales los fijó el agente y se pueden cambiar con una votación de 4 de 6. Una votación que no llega a la mayoría en el plazo se considera **rechazada**.

## 2. Mercado

| Parámetro | Valor inicial |
|---|---|
| Regulación | UE ([ESMA](../GLOSARIO.md#esma)) |
| Instrumentos operables | US Tech 100, US 500, Wall Street, Germany 40 (DAX), Euro Stoxx 50, y CFDs de acciones de la lista `watchlist` (componentes del NASDAQ 100) |
| Índices de Asia (Nikkei, Hang Seng) | **Solo contexto y análogos** en v1: su sesión es de madrugada en España y nadie podría aprobar a tiempo |
| [Timeframe](../GLOSARIO.md#timeframe) de entrada | 15 minutos |
| Timeframe de contexto | 1 hora |
| Hora interna | UTC. En pantalla: Europe/Madrid |

**Horario de entradas.** Las sesiones se definen en la hora local de cada bolsa, para que los cambios de horario de verano se resuelvan solos:

| Grupo | Sesión (hora local de la bolsa) | Equivale en Madrid (normalmente) |
|---|---|---|
| Índices y acciones de EE. UU. | 09:30–16:00 America/New_York | 15:30–22:00 |
| Índices europeos | 09:00–17:30 Europe/Berlin | 09:00–17:30 |

- No se abren operaciones en los **primeros 15 minutos** de cada sesión.
- No se abren operaciones nuevas en los **últimos 30 minutos** de la sesión del viernes.

## 3. Datos y velas

- Solo se usan [velas cerradas](../GLOSARIO.md#vela-cerrada). Una vela de 15m que empieza a las 15:30 se usa recién desde las 15:45:00.
- Las velas de 1h se construyen agregando las de 15m. La vela de 1h de las 15:00 recién existe a las 16:00.
- La señal se calcula al cierre de la vela `t`. La entrada simulada es la **apertura de `t+1`** + [slippage](../GLOSARIO.md#slippage).
- **Calentamiento:** ningún instrumento emite señales hasta tener 200 velas de 15m y 60 velas de 1h válidas.

## 4. Indicadores (definición exacta)

| Indicador | Definición |
|---|---|
| [SMA50](../GLOSARIO.md#sma) (15m y 1h) | Media aritmética de los últimos 50 cierres |
| [ATR14](../GLOSARIO.md#atr) (15m) | True Range = max(H−L, \|H−C₋₁\|, \|L−C₋₁\|), con suavizado de **Wilder** de 14 períodos (igual que TradingView) |
| [Pivote](../GLOSARIO.md#pivote) (15m y 1h) | Una vela `i` es pivote alto si su máximo es **estrictamente mayor** que el de las 3 velas anteriores y las 3 posteriores (N = 3). Se **confirma** en `i+3`; antes no existe para el sistema. El pivote bajo es igual con mínimos |

**Tendencia de 1h** (sobre la última vela de 1h cerrada):
```
pendiente = SMA50_1h[ahora] − SMA50_1h[hace 5 velas]
si cierre_1h > SMA50_1h y pendiente > 0  → ALCISTA
si cierre_1h < SMA50_1h y pendiente < 0  → BAJISTA
si no                                    → NEUTRAL
```

## 5. Señales

Cada señal tiene **dirección** (largo o corto) y pertenece a uno de dos tipos:
- **Gatillo:** tiene que ocurrir en la vela que acaba de cerrar (`t`).
- **Contexto:** describe la situación en `t`.

Abajo se describe la versión para largos. **La versión para cortos es el espejo exacto** (máximos ↔ mínimos, arriba ↔ abajo).

### 5.1 Velas (gatillo)
```
cuerpo = |C − O|;  rango = H − L;  mecha_inf = min(O,C) − L;  mecha_sup = H − max(O,C)
caída_previa = C[t−1] < C[t−6]

MARTILLO (largo):
  caída_previa
  y mecha_inf ≥ 2 × cuerpo
  y mecha_sup ≤ 0,3 × rango
  y cuerpo ≥ 0,05 × rango
  y rango ≥ 0,5 × ATR

ENVOLVENTE ALCISTA (largo):
  C[t−1] < O[t−1]                 # vela anterior bajista
  y C[t] > O[t]                   # vela actual alcista
  y O[t] ≤ C[t−1] y C[t] ≥ O[t−1]
  y cuerpo[t] > cuerpo[t−1]
```
El espejo bajista del martillo es la [estrella fugaz](../GLOSARIO.md#estrella-fugaz).

### 5.2 [Doble suelo / doble techo](../GLOSARIO.md#doble-techo) (gatillo)
```
DOBLE SUELO (largo):
  S1, S2 = dos pivotes bajos confirmados de 15m, S2 posterior a S1
  10 ≤ velas entre S1 y S2 ≤ 60
  |S1 − S2| / S1 ≤ 0,3 %                       # tolerancia entre los suelos
  neckline = máximo más alto entre S1 y S2
  neckline − min(S1,S2) ≥ 1 × ATR              # el patrón tiene altura suficiente
  gatillo: C[t] > neckline y C[t−1] ≤ neckline
           y t ≤ confirmación de S2 + 20 velas
```

### 5.3 [Soporte / resistencia](../GLOSARIO.md#soporte) (gatillo)
```
nivel de soporte = grupo de ≥ 3 pivotes bajos de 15m de las últimas 200 velas,
                   todos dentro de ±0,15 % del promedio del grupo
gatillo de rechazo (largo): L[t] ≤ nivel × (1 + 0,15 %) y C[t] > nivel
```

### 5.4 Liquidez [ICT/SMC](../GLOSARIO.md#liquidez-ict)
```
SWEEP DE LIQUIDEZ EXTERNA (gatillo, largo):
  P = último pivote bajo confirmado de 1h
  L[t] < P y C[t] > P                     # pinchó por debajo y volvió a cerrar adentro

FVG ALCISTA (contexto): en las velas i−2, i−1, i: L[i] > H[i−2]; la zona es [H[i−2], L[i]]
  Se llena (y deja de valer) cuando una vela cierra por debajo de H[i−2]
  Vale +1 si L[t] toca una zona FVG alcista sin llenar de las últimas 50 velas

ORDER BLOCK ALCISTA (contexto): la última vela bajista antes de un movimiento alcista
  cuyo cuerpo ≥ 1,5 × ATR y que produce un BOS; la zona es [L, H] de esa vela
  Vale +1 si L[t] toca un order block sin invalidar (no hubo cierre por debajo de su L)

BOS alcista (contexto): un cierre por encima del último pivote alto confirmado de 15m, con tendencia 1h ALCISTA
CHoCH alcista (contexto): lo mismo, pero con tendencia 1h BAJISTA (posible giro). Vale si ocurrió en las últimas 12 velas
```

## 6. Puntaje de confluencia y entrada

```
para cada instrumento, al cierre de cada vela de 15m, y para cada dirección:
  si no hay NINGÚN gatillo en t → no hay señal

  puntaje = 0
  tendencia 1h a favor       +2   | en contra −3 | neutral 0
  precio del lado correcto de la SMA50 de 15m   +1
  martillo o envolvente      +1
  ruptura de doble suelo     +2
  rechazo en soporte         +1
  sweep de liquidez externa  +2
  toque de FVG u order block +1
  CHoCH reciente             +1
  ajuste por noticias        de −2 a +2   (sección 9)

  si puntaje ≥ 5 y pasan TODOS los filtros (sección 7) → PROPUESTA de operación
```
Cada propuesta incluye: instrumento, dirección, entrada, stop, TP1, tamaño, apalancamiento resultante, puntaje y desglose, [probabilidad estimada](../GLOSARIO.md#probabilidad) con su rango, noticias relevantes y [análogos](../GLOSARIO.md#analogos).

**Validez para el humano:** la propuesta vence a los **10 minutos**, o antes si el precio se alejó más de **0,25R** de la entrada propuesta. Si vence, no se ejecuta (en el registro real). La simulación siempre entra en la apertura de `t+1`.

## 7. Filtros: no se abre si...

1. Estamos fuera de horario (sección 2).
2. Hay una noticia de **alto impacto** del instrumento o de su país entre **15 minutos antes y 15 minutos después**.
3. El [spread](../GLOSARIO.md#spread) actual es más de 2 veces su promedio de los últimos 20 días a esa hora.
4. Ya hay una posición abierta en ese instrumento (máximo 1 por instrumento).
5. Ya hay **5 posiciones** abiertas en total.
6. Se alcanzó la pérdida máxima del día o está activo el kill switch (sección 10).
7. La distancia al stop es menor a 0,5 × ATR o mayor a 3 × ATR.
8. Hay un error de datos (sección 12).

## 8. Stop, tamaño, apalancamiento y salidas

**[Stop loss](../GLOSARIO.md#stop-loss)** (largo):
```
estructura = el mínimo de:
   el mínimo de la vela gatillo (y de la anterior, si es envolvente)
   min(S1, S2) si el gatillo es un doble suelo
   L[t] (la mecha) si el gatillo es un sweep
   el nivel de soporte si el gatillo es un rechazo
stop = estructura − 0,5 × ATR
R = entrada − stop
```

**Tamaño y [apalancamiento](../GLOSARIO.md#apalancamiento):**
```
riesgo_€   = 1 % × capital_actual
tamaño     = riesgo_€ / R              (€ por punto, convertido a la moneda del instrumento)
nocional   = tamaño × entrada
apalanc.   = nocional / capital_actual
si apalanc. > tope del instrumento (índices 10:1, acciones 3:1):
     tamaño = tope × capital_actual / entrada     # el riesgo queda por debajo del 1 %
si el margen requerido supera el margen libre → no se abre
si el tamaño es menor al mínimo del broker → no se abre
```

**Salidas:**
```
TP1: al tocar entrada + 2R → se cierra el 50 % y el stop pasa a la entrada
Trailing (sobre el 50 % restante): stop = max(stop_actual, máximo desde TP1 − 2 × ATR)
Salida por tiempo: si a las 32 velas (8 h de mercado) no tocó +1R → se cierra al cierre de esa vela
Viernes: se cierra todo en la última vela de 15m antes del cierre de la sesión del instrumento
Si la vela anterior al cierre de la semana es un feriado, aplica el último día hábil
```
**Ambigüedad dentro de la vela (backtest):** si en una misma vela se tocan el stop y el TP, se asume que **primero se tocó el stop** (el supuesto conservador).

## 9. Noticias

| Rol | Regla |
|---|---|
| Filtro | ±15 min alrededor de eventos de impacto **alto** del calendario económico (sección 7) |
| Ajuste de probabilidad | El LLM devuelve `sentimiento ∈ [−1, 1]` y `relevancia ∈ [0, 1]` por instrumento. Ajuste = `redondear(2 × sentimiento × relevancia)`, usando solo noticias de las últimas 4 h |
| Alerta de salida | Si hay una posición abierta y llega una noticia con relevancia ≥ 0,7 y \|sentimiento\| ≥ 0,7 en contra → alerta inmediata al responsable. **No cierra sola** |
| Señal propia | **Experimental:** se registra y se mide, pero no genera propuestas hasta demostrar valor fuera de muestra (fase 5) |

- Cada noticia se guarda con su **hora de publicación real** (UTC) y solo influye desde ese momento.
- **Resúmenes en español**, con el titular original y el enlace a la fuente.
- Fuentes pedidas: Forex Factory, Investing.com, Bloomberg, Wall Street Journal, Financial Times, El Mundo, TradingView y Yahoo Finance, además de fuentes oficiales: Fed, BCE, BLS y SEC EDGAR.
  - ⚠️ Varias de estas fuentes tienen muro de pago o prohíben el acceso automatizado en sus términos de uso. En la fase 4b se define **la vía legal de acceso de cada una** (RSS oficial, API con licencia o plan pago) en un ADR. **No se hará scraping que viole los términos de uso.**

<a id="psicologia-del-mercado"></a>
### 9.1 Psicología del mercado (capa de contexto experimental, #25)

Mide miedo, euforia y los demás estados emocionales del mercado como **números objetivos**, para estudiar si anticipan el precio. Es una capa de contexto del [ADR 0003 §C](../decisions/0003-modelo-adaptativo-y-contexto.md): **se registra y se mide, pero no suma al puntaje** hasta demostrar valor fuera de muestra en la fase 5 y ganar una votación. Las fuentes son las del [ADR 0004](../decisions/0004-fuentes-de-sentimiento.md). Todos los valores de esta sección son **iniciales** y van en `config/`.

**Alcance v1:** US 500, US Tech 100 y Wall Street. Germany 40 queda afuera, porque no tiene COT y el resto de las fuentes miden el mercado de EE. UU.

#### Índice de sentimiento v1 ∈ [−1, +1]

−1 = miedo extremo, +1 = euforia extrema. Se calcula por instrumento.

```
ventana_i    = valores de la fuente i publicados en los 3 años anteriores al dato actual
               (sin incluirlo; solo lo que estaba publicado en ese momento)
percentil_i  = (valores de la ventana menores que el actual + 0,5 × valores iguales) / tamaño de la ventana
componente_i = 2 × percentil_i − 1          (o 1 − 2 × percentil_i si la fuente se invierte)
índice       = Σ peso_i × componente_i / Σ peso_i      (solo con componentes válidos)
```

| Componente | Valor crudo | Sentido | Disponible desde | Dónde se calcula |
|---|---|---|---|---|
| **COT** (TFF, futuros) | (largos − cortos de *Leveraged Funds*) / interés abierto, del mercado de `config/cot.toml` | + = euforia | su `published_at` (`config/cot.toml`) | dato crudo: Python (`sentiment`, ya guardado en `sentiment_observations`) |
| **Volatilidad realizada** | desvío estándar muestral de los últimos 20 retornos diarios, con retorno = cierre / cierre anterior − 1 | **invertido** (alta = miedo) | fin de la vela diaria (`daily_candles`) | Rust (`indicators`) |
| **Caída desde el máximo** | (cierre − máximo de las últimas 252 velas diarias, incluida la actual) / ese máximo. Vale ≤ 0 | + (más negativo = más miedo; no se invierte) | fin de la vela diaria | Rust (`indicators`) |
| Amplitud | % de componentes del NASDAQ 100 **de esa fecha** con cierre por encima de su SMA50 | + = euforia | fin de la vela diaria | Rust. **Pendiente de datos** (precios de componentes, ADR 0005). Solo US Tech 100 |
| Nuevos máximos y mínimos | (componentes en máximo de 52 semanas − en mínimo de 52 semanas) / total | + = euforia | fin de la vela diaria | Rust. **Pendiente de datos**, como la amplitud |

- **Qué cierre se usa:** el de la vela diaria de IG, que va de **medianoche a medianoche de Madrid** (`docs/research/ig-velas-diarias.md`), no el cierre de la NYSE. No cambia las reglas, pero los valores no se comparan con un cierre de Nueva York.
- **Combinación:** el percentil, el componente y el índice se calculan en Python (`intelligence/cfds_intel/sentiment`), leyendo los valores crudos de PostgreSQL. Las medidas que salen del precio se calculan **solo en Rust** (CLAUDE.md §1).
- **Pesos iniciales:** iguales. **Mínimo de componentes válidos:** 3. Con las tres fuentes que hoy tienen datos, el índice v1 necesita las tres.
- **Componente válido:** tiene al menos 3 años de historia publicada (calentamiento) y su último dato no está vencido. Vencimiento inicial: COT 21 días, medidas diarias 5 días.
- ⚠️ **Pendiente de decidir por el dueño (modo de un solo constructor), antes de mirar datos (revisión del PR #133):** con 5 años de diarias en el Mac mini (#102, desde 2021-11) y 3 años de calentamiento, el índice existe recién desde fines de 2024. Quedan unos 2 años, de los que además sale el tramo fuera de muestra, y con un horizonte de 20 días los quintiles extremos de H1 y H2 probablemente den "datos insuficientes". Opciones:
  1. **Cargar más historia diaria** de US 500, US Tech 100 y Wall Street: 5 años más son unos 1.300 puntos de IG por índice, una sola vez, en el Mac mini. Mantiene los 3 años.
  2. **Bajar el calentamiento**, por ejemplo a 2 años, dejándolo escrito antes de mirar datos. Más muestra, pero cada percentil se compara con una historia más corta.
  3. **Las dos:** más historia y además un calentamiento más corto.

  Hasta que se decida, vale lo escrito (3 años), y no se analiza ningún dato de H1 ni H2.
- **Cuándo se recalcula:** cada vez que un componente publica un dato nuevo. La hora del índice es la del dato más reciente que usa.
- **Zonas:** **miedo extremo ≤ −0,6** y **euforia extrema ≥ +0,6**.
- **Sin repintado:** un valor publicado no cambia. Si la fuente corrige un dato, se guarda como dato nuevo con su propia hora de publicación.
- **Fuera del índice v1:**
  - **% de clientes de IG largos:** no tiene historia. La serie que se conserva es la del Mac mini, que se graba desde el **22-11-2026** (`cron` cada 15 min, #124); lo grabado antes en una sesión en la nube se perdió. Se usa solo en H3, con umbrales absolutos, contando desde el 22-11-2026.
  - **Sentimiento de noticias (§9):** entra cuando tenga 3 años de interpretaciones grabadas.
  - **VIX, put/call y AAII:** quedan afuera por términos de uso (ADR 0004).
  - **X (ex Twitter):** afuera por ahora. El [ADR 0007](../decisions/0007-x-como-fuente-de-sentimiento.md) (propuesta) recomienda no incorporarlo: es pago y hay dudas sobre sus términos.
- **Contrato:** `sentiment.v1` (nuevo, no `context_event.v1`, que es para eventos con fecha). Lleva instrumento, hora del dato, `published_at`, valor, cada componente usado con su valor crudo y su percentil, y si terminó el calentamiento. Se define en el PR que lo implemente, antes del código.
- **Cómo lo usa Rust:** solo lo guarda en el diario con cada propuesta (§15b). No toca el puntaje.

#### Estados emocionales (definiciones objetivas)

Lo que sale del precio se calcula en Rust, dentro de los módulos que ya existen. Lo externo, en Python. **No hay un módulo `market_mood`:** si algún estado demuestra valor, la combinación la hace el puntaje de `strategy`, como con las demás señales.

| Estado | Regla (al cierre de la vela, solo velas cerradas) | Timeframe | Dónde |
|---|---|---|---|
| **FOMO de precio** | (cierre − EMA20) / ATR14 ≥ 3 **y** el cierre es el más alto de las últimas 20 velas | 1h | Rust (`regime`) |
| **Pánico / clímax de venta** | (EMA20 − cierre) / ATR14 ≥ 3 **y** el cierre es el más bajo de las últimas 20 velas | 1h | Rust (`regime`) |
| **Rechazo** | Martillo o estrella fugaz (§5.1) cuya mecha toca un soporte o una resistencia (§5.3), un FVG o un order block (§5.4); o un sweep de liquidez (§5.4) que cierra de vuelta dentro del rango. No es un detector nuevo: reúne los que ya existen | 15m y 1h | Rust (`patterns` y `liquidity`) |
| **Distribución** | Día de distribución: cierre diario ≥ 0,2 % por debajo del anterior, con más volumen que el día anterior. Estado: ≥ 5 días de distribución en las últimas 25 sesiones | diario | Rust (`regime`). **Pendiente de volumen real** |
| **Miedo extremo** | Índice ≤ −0,6 | según el índice | Python (`sentiment`) |
| **Euforia extrema** | Índice ≥ +0,6 | según el índice | Python (`sentiment`) |
| **Complacencia** | Percentil de la volatilidad realizada ≤ 0,10 | diario | Python (`sentiment`) |

- EMA20 de 1h: media exponencial de 20 períodos con α = 2 / 21, que arranca con la SMA de los primeros 20 cierres. ATR14 de 1h: la definición de Wilder de §4, aplicada a velas de 1h. Cada regla tiene su calentamiento: no se evalúa sin 20 velas válidas, además de lo que necesite cada indicador.
- **Volumen:** IG no informa volumen de mercado en sus CFD (el campo queda `null`). Por eso FOMO y clímax se definen **sin volumen** en v1, y la distribución no se calcula hasta tener volumen real del subyacente con una fuente legal (ADR 0004 y #10). Si después se le suma una condición de volumen a un estado, es un estado **nuevo**, con su propia hipótesis registrada, y no un cambio del anterior.

#### Hipótesis registradas antes de mirar los datos

Para todas: "la media" es el retorno medio del mismo instrumento y horizonte en el mismo tramo de datos. Una hipótesis se da por **apoyada** solo si el intervalo de confianza del 95 % (bootstrap) de la diferencia contra la media no incluye el 0. Retornos de cierre a cierre, sin costos (la operativa con costos se mide aparte, en la fase 5).

| | Condición | Instrumentos | Se espera |
|---|---|---|---|
| **H1** (miedo, al revés) | Índice ≤ −0,6 | US 500, US Tech 100, Wall Street | retorno a 20 días hábiles **mayor** que la media |
| **H2** (euforia, al revés) | Índice ≥ +0,6 | ídem | retorno a 20 días hábiles **menor** que la media |
| **H3** (posicionamiento de IG) | ≥ 75 % de clientes de IG largos (o ≥ 75 % cortos) | los instrumentos con dato de IG | retorno medio a 1 y a 5 días hábiles **en contra de la mayoría** |
| **H4** (FOMO) | FOMO de precio en 1h | US 500, US Tech 100, Wall Street | retorno a 4 h **mayor** que la media (continúa) **y** retorno a 5 días hábiles **menor** que la media |
| **H5** (capitulación) | Clímax de venta en 1h con índice ≤ −0,6 | ídem | retorno a 5 días hábiles **mayor** que la media |
| **H6** (rechazo) | Rechazo confirmado, con stop estructural (§8) y objetivo en el nivel opuesto más cercano (§5.3) | todos los operables | toca el objetivo antes que el stop **con más frecuencia** que una entrada aleatoria con el mismo stop, objetivo y frecuencia |
| **H7** (distribución) | Estado de distribución | US 500, US Tech 100, Wall Street | retorno a 20 días hábiles **menor** que la media. **No se prueba hasta tener volumen real** |

**Cómo se validan (fase 5, no antes):**
- Retornos futuros agrupados por quintil del índice, con su intervalo de confianza. Si un grupo tiene menos de 30 casos: "datos insuficientes".
- Walk-forward, con el período fuera de muestra sin tocar, comparando contra comprar y mantener y contra entradas aleatorias (ADR 0003 §D).
- **Cada hipótesis, horizonte e instrumento probado va al registro de experimentos**, y el Sharpe deflactado los cuenta a todos. Son muchas combinaciones: ese es el principal riesgo de sobreajuste de esta capa.
- **Si aporta**, entra al puntaje con un peso de **±1 como máximo**, sin superar el ±2 del ajuste de noticias, y solo después de votarse (ADR 0003 §C.3).

**Limitación aceptada:** el sentimiento actúa en horizontes de días o semanas, no en velas de 15 minutos. Su uso esperado es como **filtro o ajuste del puntaje**, nunca como gatillo de entrada. Las medidas propias salen del precio, así que pueden repetir lo que la estrategia ya ve. Por eso se comparan contra no usarlas.

## 10. Límites de riesgo (módulo independiente, la estrategia no los puede saltear)

| Límite | Valor | Qué pasa |
|---|---|---|
| Riesgo por operación | 1 % | Se limita el tamaño |
| Pérdida diaria (realizada + no realizada, desde las 00:00 Madrid) | −3 % | No se abren operaciones hasta el día siguiente |
| [Drawdown](../GLOSARIO.md#drawdown) desde el máximo del fondo | −15 % | **[Kill switch](../GLOSARIO.md#kill-switch):** no se proponen operaciones y se alerta. Se reactiva solo por votación (sección 1.2) |
| Posiciones abiertas | 5 | No se abren más |
| Posiciones por instrumento | 1 | No se abren más |
| Apalancamiento por posición | índices 10:1, acciones 3:1 | Se reduce el tamaño |
| [Stop-out](../GLOSARIO.md#stop-out) del broker | 50 % del margen (mínimo ESMA) | La simulación replica el cierre forzado |

## 11. Costos (nunca en cero)

Por instrumento, en `config/`: [spread](../GLOSARIO.md#spread) por franja horaria, [comisión](../GLOSARIO.md#comision), [swap](../GLOSARIO.md#swap) diario de largos y de cortos (el triple los miércoles o según el broker), y [slippage](../GLOSARIO.md#slippage) supuesto (inicial: 0,1 × spread promedio + 1 tick).
⏳ **Los valores reales salen de la documentación oficial del broker elegido** (tarea de la fase 2). No se inventan. Hasta tenerlos, el backtest no se considera válido.

## 12. Casos borde

| Caso | Regla |
|---|---|
| Faltan datos, hay datos tardíos o hay un error de conexión | No se proponen operaciones nuevas en ese instrumento. Se alerta. Las posiciones siguen con su stop puesto en el broker |
| [Gap](../GLOSARIO.md#gap) que salta el stop o el TP | Se ejecuta al **precio de apertura** de la vela del gap, no al nivel de la orden |
| Feriado | Calendario por bolsa: ese instrumento no opera, y el cierre del viernes se adelanta al último día hábil |
| [Halt](../GLOSARIO.md#halt) | No se proponen operaciones nuevas en el instrumento. La posición abierta se marca a riesgo y se alerta |
| Ejecución parcial (registro real) | El responsable registra lo ejecutado y el stop y el TP se ajustan al tamaño real |
| Datos sospechosos (precio ≤ 0, salto > 10 × ATR sin gap de sesión, duplicados, orden temporal roto) | Se descartan, se alerta y se aplica la primera fila de esta tabla |

## 13. Probabilidad y [análogos](../GLOSARIO.md#analogos)

- **Probabilidad mostrada** = frecuencia histórica de "toca TP1 antes que el stop" en propuestas con el mismo tipo de gatillo y el mismo rango de puntaje, medida en el backtest de entrenamiento y validación, **con su intervalo de confianza**. Si hay menos de 30 casos, se muestra "datos insuficientes".
- **Análogos (v1: solo informativos, no suman al puntaje).** El objetivo final es un universo de índices + componentes del NASDAQ 100, 5 años, **incluidas las empresas que salieron del índice**. La **v1** se limita a lo que se puede tener sin costo (ADR 0005 §4, #148):

  **Universo y datos.** Las velas **diarias** de IG de los índices operables (DE40, US100, US500 y US30), guardadas en `daily_candles`. Solo cuentan los días con sesión de la bolsa de cada índice (`config/sessions.toml`): las velas que IG manda en fines de semana o feriados se ignoran. Un índice sin al menos `ventana + 20` diarias guardadas no participa. Los componentes del NASDAQ 100 se suman cuando haya una fuente de precios aprobada (#10).

  **1. Forma del gráfico (único grupo de v1).**
  ```
  ventana      = las últimas N = 20 velas diarias cerradas (consulta y candidatos)
  normalización = z-score de los cierres de la ventana: (cierre − media) / desvío
                  (desvío muestral; si es 0, la ventana no se usa)
  distancia    = DTW con banda de Sakoe-Chiba de 2 velas, costo |a − b|
  vecinos      = los K = 20 candidatos más cercanos, de cualquiera de los índices
  ```
  - **Sin look-ahead:** un candidato vale solo si su **resultado completo** ya existía al cierre de la consulta. Es decir, la última vela de su horizonte más largo (20 sesiones) cerró antes que la última vela de la ventana consultada.
  - **Sin repetir el mismo episodio:** dentro de un mismo índice, dos vecinos elegidos no pueden tener ventanas que se superpongan. Se recorren de menor a mayor distancia y se saltean los que se superponen con uno ya elegido. La propia ventana consultada y las que se superponen con ella tampoco cuentan.
  - Con menos de 20 vecinos válidos se muestra **"datos insuficientes"**.

  **Qué pasó después** (de cada vecino, desde el cierre de su última vela):
  - Retorno a **1, 5 y 20 sesiones** (cierre / cierre − 1). Se muestran los percentiles 10, 25, 50, 75 y 90 y el % de retornos positivos.
  - **% que tocó +2R antes que −1R** en las 20 sesiones siguientes, con `R = ATR14 diario` al cierre de la ventana (la misma definición de Wilder de §4, sobre velas diarias). Se miran el máximo y el mínimo de cada vela siguiente. Si una misma vela toca los dos, cuenta como **−1R** (conservador). Si no toca ninguno en 20 sesiones, cuenta aparte, como "ninguno".

  **2. Mismo patrón técnico (el mismo gatillo, con la misma tendencia de 1h): pendiente.** Necesita historia de 15m y 1h de los cuatro índices, que no está cargada (cupo de IG). Se define cuando la haya.

  **3. Contexto macro (sin VIX).** El VIX quedó fuera por sus términos de uso (ADR 0004). En v1, el contexto de cada ventana (consulta y vecinos) es:
  - **Volatilidad:** la volatilidad realizada del índice (desvío estándar muestral de los últimos 20 retornos diarios, con retorno = cierre / cierre anterior − 1), en tercios de su propia historia de los 3 años anteriores: **baja / media / alta**. Sin 3 años de historia, "sin dato".
  - **Tasas:** la variación del tipo de referencia de financiación del índice (`[funding_benchmarks]`; hoy el €STR, #79) en las últimas 60 sesiones: **sube** (> +0,10 pp), **baja** (< −0,10 pp) o **estable**. Se usa el valor publicado hasta ese día. Los índices sin tipo de referencia confirmado (#67) quedan "sin dato".
  - Además del resultado de los 20 vecinos, se muestra el de una **segunda búsqueda** con las mismas reglas, pero solo entre los candidatos que tienen **el mismo contexto** (volatilidad y tasas) que la consulta, con su propio "datos insuficientes" si son menos de 20. Si a la consulta le falta algún dato de contexto, este grupo queda vacío.

  **Dónde se calcula (CLAUDE.md §1).** El **ATR14 diario** y la **volatilidad realizada** son indicadores: los calcula **Rust** (`indicators`) sobre `daily_candles` y los guarda para que los lea Python, con un contrato nuevo (contratos primero). La búsqueda de análogos (z-score, DTW, vecinos, contexto y resultados) vive en **Python** (`intelligence/cfds_intel/analogs`) y no recalcula ningún indicador.

  Todos los números de este punto (N, banda, K, horizontes, R, umbrales) van en `config/` y son **iniciales**: se fijan antes de mirar resultados y no se ajustan con el período fuera de muestra.
- Los métodos son **probabilísticos** (frecuencias, intervalos de confianza, Monte Carlo para el riesgo del fondo). No se usa computación cuántica.
- Las opciones (volatilidad implícita, put/call, VIX) son **solo contexto**.
- **Psicología del mercado (§9.1): solo informativa en v1.** La propuesta muestra el índice de sentimiento con su zona y los estados emocionales activos, y se guardan en el diario. No cambian la probabilidad mostrada ni el puntaje hasta que la fase 5 lo valide y se vote.

## 14. Fondo del grupo

```
moneda: EUR · 6 miembros · titular: 1 persona · decisiones: sistema + responsables
NAV = valor total del fondo / unidades emitidas
aportes: lunes, antes de la apertura europea, al NAV del cierre del viernes; mínimo 10 €
retiros: se piden antes del lunes y se pagan al NAV del cierre del viernes
P&L por miembro = unidades × NAV − (aportes − retiros)
capital simulado inicial: 1.000 €
```
Invariante: la suma del valor de todos los miembros es igual al valor total del fondo, al céntimo.

## 15. Validación y paso a dinero real

- Backtest: los últimos 12 meses de datos quedan **fuera de muestra** y no se tocan hasta la fase 5. Validación **walk-forward**.
- Paper trading: mínimo **3 meses y 50 operaciones**, comparado contra el backtest del mismo período.
- Dinero real: solo con un ADR aprobado por los dos colaboradores, después de un asesoramiento legal e impositivo sobre el fondo, y empezando con capital mínimo.

## 15b. Aprendizaje continuo

El sistema aprende de sus resultados, pero **nunca cambia solo sus propios parámetros**: si lo hiciera, terminaría ajustándose al ruido del pasado ([sobreajuste](../GLOSARIO.md#sobreajuste)) y saltearía la decisión del grupo.

**Diario** (módulo `journal`): cada propuesta se guarda con
- instrumento, dirección, hora, entrada, stop, TP1, tamaño y apalancamiento;
- puntaje y su desglose por señal, noticias usadas, análogos y probabilidad mostrada;
- qué hizo el responsable (aprobó, rechazó o la dejó vencer, con quién y cuándo);
- el resultado simulado y, si se ejecutó, el real: P/L, si tocó TP1 o el stop, y el motivo de salida.

**Informe mensual** (y a pedido), con datos del diario:
1. **Calibración:** por rango de probabilidad mostrada (por ejemplo 50–60%), frecuencia real con su intervalo de confianza. Si no coinciden, se propone recalibrar. Con menos de 30 casos en un rango: "datos insuficientes".
2. **Aporte por señal:** resultado de las propuestas con y sin cada señal (velas, doble techo o suelo, soportes, liquidez, noticias). Las que no aportan se proponen para bajar de peso o sacar.
3. **Simulado contra real:** diferencias de precio, slippage y propuestas vencidas sin ejecutar.
4. **Modelos (Python):** reentrenamiento con walk-forward, sin tocar los datos fuera de muestra. Un modelo nuevo se propone solo si mejora fuera de muestra al anterior.

**Del informe al cambio:** cada propuesta de cambio llega a la app como **votación** (cambio de reglas: 4 de 6, spec §1.2). Si se aprueba, se aplica en `config/` con el voto registrado; si no, no cambia nada.

## 16. Qué decidió el grupo y qué decidió el agente

**Lo decidió el grupo:** instrumentos, país y regulación, timeframes 15m/1h, cierre los viernes, rol del sistema (propone + responsable aprueba), señales de entrada, entrada en la ruptura del neckline, marco ICT/SMC, puntaje de confluencia, stop por estructura + ATR, TP parcial en 2R + trailing, perfil de riesgo moderado, apalancamiento que sale del riesgo, los 4 roles de las noticias, el LLM para interpretarlas, ventana de ±15 min, fuentes de noticias, modelos probabilísticos, horario de sesiones, datos gratuitos primero, tipos de análogos, universo de 5 años, idioma, capital de 1.000 €, aportes de los lunes con mínimo de 10 €, opciones solo como contexto, 3 meses de paper trading, titular único y retiros semanales. También la psicología del mercado como capa de contexto (§9.1), con las fuentes del ADR 0004, alcance solo en EE. UU. en v1, el contrato `sentiment.v1` y los estados de precio dentro de los módulos que ya existen (#25, 22-11-2026).

**Lo decidió el agente** (el grupo no es experto en lo técnico; se puede revisar en cualquier momento, y la fase 5 lo valida con datos):
- Todos los parámetros técnicos: N = 3 de los pivotes, tolerancias, ATR14 Wilder, pesos del puntaje, umbral 5, 10 minutos de validez, tope de stop de 0,5–3 ATR, trailing a 2 ATR.
- Reglas de casos borde (sección 12) y salida por tiempo a las 8 h.
- Topes de apalancamiento propios (10:1 y 3:1, por debajo de ESMA) y stop-out al 50 %.
- Filtros extra: primeros 15 minutos de sesión, spread anormal, 1 posición por instrumento.
- Índices de Asia solo como contexto en v1.
- Análogos solo informativos en v1, y la noticia como señal propia en modo experimental.
- Las reglas de §9.1: la serie del COT (*Leveraged Funds* netos sobre interés abierto), las ventanas de 20 y 252 días, la EMA20 y el umbral de 3 ATR del FOMO y del clímax, el percentil de 0,10 de la complacencia, los 5 días de distribución en 25 sesiones, los vencimientos y los horizontes de H1–H7.

## 17. Supuestos y riesgos

- **Supuesto:** el broker final está en la UE con ESMA y sus CFDs replican los índices de contado. Se confirma con un ADR en la fase 2.
- **Supuesto:** con datos gratuitos puede haber diferencias contra los precios del CFD del broker. Se mide en la fase 2.
- **Riesgo:** las reglas ICT/SMC son una traducción objetiva de conceptos que cada trader interpreta distinto. Si no coinciden con lo que ustedes ven en el gráfico, se ajusta la definición, no el código a ojo.
- **Riesgo:** muchas señales combinadas aumentan el riesgo de [sobreajuste](../GLOSARIO.md#sobreajuste). Por eso el puntaje es simple y los pesos solo cambian con evidencia fuera de muestra.
- **Riesgo legal:** el fondo de 6 personas con un titular (ver la nota en las preguntas).
- **Sin garantía:** ninguna probabilidad mostrada es una promesa de ganancia.
