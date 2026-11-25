# Programa 6: pre-registro (borrador). Tendencia en 16 mercados con riesgo por volatilidad

> **Estado: BORRADOR, sin congelar** (Issue #672). Escrito por un agente de IA (Claude Code) el 25-11-2026, por el punto 3 del plan del autoanálisis (`2026-11-autoanalisis.md` §9 y §10, #658). Pedido del dueño, 25-11-2026: «Daleeee arrancaaa con todo».
>
> **No hay `.sha256` ni cambios de configuración.** Mientras sea borrador **no se corre nada** con estas reglas. Todas las elecciones están en la tabla de §15, en **PENDIENTE**, con el valor recomendado y por qué. Congelar pide la aprobación explícita del dueño, citada con sus palabras y la fecha.
>
> Antes de escribir esto solo se **contaron** velas, vencimientos y huecos (§3.2), con una conexión de solo lectura. **No se calculó ningún retorno, ninguna señal, ningún P/L ni ningún resultado** de estos 16 mercados.
>
> Método y formato: los pre-registros del programa 4 (`2026-11-programa-4-preregistro.md`) y del programa 5 (`2026-11-programa-5-fvg-preregistro.md`). Datos de IG y de Databento: los anexos B y C del borrador del programa 3 (`2026-11-programa-3-borrador.md`), que no se repiten acá.

Nada de esto es una recomendación de operar. Los resultados pasados no prometen ningún resultado futuro. Nada de esto toca dinero real (`CLAUDE.md` §5): aunque la hipótesis pase, queda en simulación.

## Resumen simple

**Qué es.** El "seguimiento de tendencia" compra lo que viene subiendo y vende lo que viene bajando, en muchos mercados a la vez (bolsas, bonos, monedas y materias primas). Cada mercado recibe una posición más chica cuanto más se mueve, para que ninguno domine. Es la idea con más evidencia publicada en el trading sistemático (Moskowitz, Ooi y Pedersen, 2012; Hurst, Ooi y Pedersen, AQR). Robert Carver publicó una versión con reglas fijas (`pysystemtrade`).

**Qué se quiere saber.**

1. **H6.1 (la hipótesis que vota).** Una cartera de tendencia estilo Carver (tres medias móviles exponenciales cruzadas y una regla de ruptura, con parámetros publicados, sin ajustar nada) sobre 16 futuros diarios, ¿da un retorno ajustado por riesgo positivo y que **agrega** algo a tener el S&P 500, después de costos?
2. **H6.0 (el control, sin voto).** La regla más simple de la literatura: largo si el mercado subió en los últimos 12 meses, corto si bajó. Sirve para saber si H6.1 agrega algo sobre lo simple.

**Qué no es el criterio.** No se pide "ganarle al S&P 500 en retorno". Una cartera de tendencia rara vez lo hace en una década alcista. Lo que se pide es que tenga **ventaja propia** (alfa) frente al S&P 500 y que **no se hunda cuando la bolsa se hunde** (§7).

**Por qué ahora.** En velas de 15 minutos los costos se comen entre 0,10 y 0,18 R por operación (autoanálisis §4). En diario, con posiciones que duran semanas, el costo pesa mucho menos. Y los datos ya están descargados (§3).

**Lo que hay que decidir primero (§0):** este programa trata casi lo mismo que el **programa 3** (borrador del dueño, #513). No conviene correr los dos.

## 0. Relación con el programa 3 (#513)

El programa 3 es el borrador del dueño (24-11-2026) de **tendencia multi-activo** sobre casi el mismo universo, sin congelar y con 11 decisiones pendientes (su §D). Las diferencias:

| | Programa 3 (borrador del dueño) | Programa 6 (este borrador) |
|---|---|---|
| Señal | Promedio de los signos de 1, 3 y 12 meses, revisada una vez por mes | EWMAC de 3 velocidades más ruptura, revisada cada día, con fuerza de la señal (tope ±20) |
| Riesgo | Mismo riesgo por mercado (2,5 % / σ) | Mismo riesgo por **clase de activo**, volatilidad objetivo de la cartera |
| Vehículo | CFD de futuros de IG | Futuros (principal) y CFD de IG (segundo escenario) |
| Prueba | Alfa contra ES+NQ+YM | A elegir (§7); recomendado: alfa contra US500 |
| Universo | 16 mercados (con FESX y FGBL, sin YM) | Los 16 de `config/databento_daily.toml` y `config/databento.toml` (con YM, sin FESX ni FGBL; FDAX queda fuera por datos, §3.2) |

**Por qué no correr los dos:** son dos pruebas muy correlacionadas sobre los mismos datos y el mismo período. Correr las dos gasta presupuesto dos veces y, si una pasa y la otra no, invita a quedarse con la que dio mejor (que es justo lo que el pre-registro quiere evitar).

**Recomendación (punto abierto 1):** que el programa 6 **reemplace** al programa 3. El programa 3 se cierra sin congelar; sus anexos B y C (Databento e IG) y las decisiones de su §D que siguen valiendo se trasladan acá (puntos 22 a 26). Otra opción es al revés: seguir con el programa 3 y descartar este borrador.

## 1. Presupuesto de error y cantidad de pruebas

> **Nota del 25-11-2026 (#690):** la regla de #660 fue aprobada por el dueño el 25-11-2026 en #660 (cita: «1, 2 y 7 apruebo la recomendacion.»), adoptada en #690. **α₆ sale de ese presupuesto global**: al congelar, H6.1 apuesta `α₆ = mín(1,25 %, W/(1 + W))` (o menos) del libro de `registro-de-resultados.md` ("Presupuesto global"), paga `α₆/(1 − α₆)` y el pre-registro cita `W` antes y después. Con `W = 4,75 %` hoy, **α₆ ≤ 1,25 %** (no 2,5 %). La opción provisional de abajo (5 % nuevo, α₆ = 0,025 y 2,5 % en reserva) **deja de existir**. Antes de congelar, H6.1 tiene que pasar el filtro de borde bruto (k = 2). Los demás puntos abiertos de §15 no cambian.

- **Presupuesto:** lo fija la regla de presupuesto global que propone #660 (`docs/analysis/2026-11-regla-presupuesto-global.md`, en preparación por otro agente; no se repite acá). El nivel de H6.1 se escribe como **α₆** y queda **PENDIENTE** hasta que el dueño apruebe esa regla (punto abierto 2).
- **Si el dueño quiere congelar antes de #660:** la recomendación provisional es la regla vieja (un 5 % nuevo para el programa) con **α₆ = 0,025** para H6.1, unilateral, y el 2,5 % restante en reserva para un programa de *carry* o de reversión diaria sobre el mismo universo, con su propio pre-registro. H6.0 no vota, así que no gasta presupuesto.
- **Cantidad de pruebas para el Sharpe deflactado:** el N del registro **al congelar**. Hoy es **361** (`config/research.toml`, `[rule_program5]`). Al congelar se suman H6.1 y H6.0 (aunque H6.0 no vote, se mira): **N = 363**, salvo que antes se congele otro programa o que #660 cambie cómo se cuenta (punto abierto 3). Referencia con N = 100 y N = 1000, sin voto.

## 2. Lo que viene de los programas anteriores sin cambios

- **Período de evaluación:** **01-02-2013 → 01-11-2025** (153 meses), el mismo de las 51 ventanas de validación de `config/data_split.toml`. 2011 y 2012 se usan solo para el calentamiento (la media más lenta necesita 256 días).
- **Sin parámetros que se estimen con los datos** (§4). Por eso no hace falta reentrenar por ventana: la cartera corre **de forma continua** del 03-01-2011 al 31-10-2025 y las estadísticas usan solo los 153 meses de evaluación. Si el dueño prefiere estimar algo (por ejemplo el multiplicador de diversificación, punto 12), se estima solo con los 24 meses de entrenamiento de cada ventana.
- **Fuera de muestra:** desde el **01-11-2025** no se carga, no se mira y no se usa (las consultas llevan `start_at < '2025-11-01'`). Las posiciones abiertas se valúan al último cierre antes de esa fecha. Usarlo pide la aprobación del dueño, una sola vez, con la regla congelada.
- **Mitades:** **02-2013 → 12-2020** y **01-2021 → 10-2025** (`second_half_from = "2021-01-01"`).
- **Estrés de costos:** × 1,5 vota; × 2 solo se informa.
- **Sharpe deflactado** (`research::stats::deflated_sharpe`), umbral **0,95**.
- **Test de suerte** con SplitMix64, semilla **20261125** (programa 5 §6).
- Estadística en `f64`; precios, P/L, costos y capital en `Decimal`; horas en UTC.

## 3. Datos

### 3.1 Fuentes

- **10 mercados con velas diarias** en `futures_bars_1d` (`config/databento_daily.toml`, #522 y #524): NKD, ZN, ZB, 6E, 6J, 6B, 6A, SI, CL y HG, todos `v.0` (el vencimiento de más volumen) de CME Globex.
- **6 mercados con velas de 1 minuto** en `futures_bars_1m` (`config/databento.toml`): ES, NQ, YM y RTY (`c.0`, el vencimiento más cercano), GC (`v.0`) y FDAX (`c.0`, Eurex). Para este programa se arman velas diarias con la **misma definición** que las de Databento (§3.3).
- Cada vela trae su `contract_id`: un cambio de `contract_id` es un **vencimiento** (renovación).
- **Lo que no hay:** velas a la hora de liquidación (*settlement*) de cada bolsa; las velas `ohlcv-1d` van de medianoche a medianoche UTC (`config/databento_daily.toml`). Precios ajustados hacia atrás: no; se arman en §3.4.

### 3.2 Conteo previo (solo cantidades, sin precios)

Contado el **25-11-2026** por el agente con un script de un solo uso (Python, conexión con `read_only`), sobre todo lo que hay en la base. Solo cuenta velas, contratos y días entre velas.

| Mercado | Clase | Tabla | Velas diarias (días con velas) | Desde | Hasta | Contratos | Cambios de contrato | Hueco máx. (días) | Huecos > 4 días | Velas de domingo | Velas antes del 01-11-2025 | Días hábiles 02-2013 → 10-2025 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ES | Renta variable | 1m (`c.0`) | 4.930 días | 2011-01-02 | 2026-11-22 | — | 63 | 4 | 0 | — | — | — |
| NQ | Renta variable | 1m (`c.0`) | 4.930 días | 2011-01-02 | 2026-11-22 | — | 63 | 4 | 0 | — | — | — |
| YM | Renta variable | 1m (`c.0`) | 4.935 días | 2011-01-02 | 2026-11-22 | — | 63 | 4 | 0 | — | — | — |
| RTY | Renta variable | 1m (`c.0`) | 2.920 días | **2017-07-09** | 2026-11-22 | — | 37 | 3 | 0 | — | — | — |
| FDAX | Renta variable | 1m (`c.0`) | **431 días** | **2025-03-11** | 2026-11-20 | — | 57 | 6 | 4 | — | — | — |
| NKD | Renta variable | 1d (`v.0`) | 4.930 | 2011-01-03 | 2026-11-23 | 64 | 65 (2 vuelven a un contrato anterior) | 4 | 0 | 812 | 4.598 | 3.303 |
| ZN | Tasas | 1d (`v.0`) | 4.936 | 2011-01-02 | 2026-11-23 | 64 | 63 | 4 | 0 | 813 | 4.604 | 3.308 |
| ZB | Tasas | 1d (`v.0`) | 4.936 | 2011-01-02 | 2026-11-23 | 64 | 63 | 4 | 0 | 813 | 4.604 | 3.308 |
| 6E | Divisas | 1d (`v.0`) | 4.931 | 2011-01-02 | 2026-11-23 | 64 | 63 | 4 | 0 | 813 | 4.599 | 3.303 |
| 6J | Divisas | 1d (`v.0`) | 4.931 | 2011-01-02 | 2026-11-23 | 64 | 63 | 4 | 0 | 813 | 4.599 | 3.303 |
| 6B | Divisas | 1d (`v.0`) | 4.931 | 2011-01-02 | 2026-11-23 | 64 | 63 | 4 | 0 | 813 | 4.599 | 3.303 |
| 6A | Divisas | 1d (`v.0`) | 4.931 | 2011-01-02 | 2026-11-23 | 64 | 63 | 4 | 0 | 813 | 4.599 | 3.303 |
| GC | Materias primas | 1m (`v.0`) | 4.932 días | 2011-01-02 | 2026-11-22 | — | 79 | 4 | 0 | — | — | — |
| SI | Materias primas | 1d (`v.0`) | 4.933 | 2011-01-02 | 2026-11-23 | 80 | 81 (2 vuelven a un contrato anterior) | 4 | 0 | 813 | 4.602 | 3.303 |
| HG | Materias primas | 1d (`v.0`) | 4.933 | 2011-01-02 | 2026-11-23 | 80 | 79 | 4 | 0 | 813 | 4.602 | 3.303 |
| CL | Materias primas | 1d (`v.0`) | 4.935 | 2011-01-02 | 2026-11-23 | 192 | 191 | 4 | 0 | 814 | 4.604 | 3.304 |

Notas del conteo:

- En las de 1 minuto, "días" son los días UTC con al menos una vela de 1 minuto desde el 2011-01-01, y los cambios de contrato se cuentan día a día. Las columnas con "—" se cuentan al armar las velas diarias (§3.3).
- **Precios ≤ 0:** ninguno en ninguna de las dos tablas (incluido CL en abril de 2020: el `v.0` ya estaba en otro vencimiento).
- **Huecos:** el mayor es de 4 días corridos (fines de semana largos) en todos menos FDAX. No hay huecos de datos dentro del período.
- **Velas de domingo:** unas 813 por mercado. Son la primera hora de la sesión de CME, que abre el domingo a la tarde en EE. UU. (§3.3).
- **Vencimientos que vuelven atrás:** en NKD y SI el `v.0` (el de más volumen) volvió 2 veces a un contrato anterior. Cada cambio se trata como un vencimiento más (§3.4), y se paga su costo (conservador).
- Las velas del fuera de muestra **se contaron** (columnas "Hasta"), pero sus precios no se leyeron.

**Lo que dice el conteo para el universo:**

- **FDAX no entra en la práctica:** empieza el 11-03-2025 y antes del 01-11-2025 tiene menos de 8 meses, menos que el calentamiento de 256 días. Databento solo tiene Eurex desde el 10-03-2025 (programa 3, anexo B). Se propone dejarlo fuera y decirlo (punto abierto 5).
- **RTY entra tarde:** desde julio de 2017 (antes el Russell estaba en ICE). Con 256 días de calentamiento, empieza a operar a mediados de 2018. Mientras no está, su peso se reparte entre los demás de su clase (§4.5).
- **Universo efectivo: 15 mercados** (14 desde 2013 y RTY desde 2018), en 4 clases: renta variable 5 (ES, NQ, YM, RTY, NKD), tasas 2 (ZN, ZB), divisas 4 (6E, 6J, 6B, 6A) y materias primas 4 (GC, SI, HG, CL).

### 3.3 La vela diaria

- **Definición:** la de Databento `ohlcv-1d`: de 00:00 a 24:00 UTC. Para los 6 mercados de 1 minuto se arma igual (apertura de la primera vela de 1 minuto del día UTC, cierre de la última, máximo, mínimo, volumen y el `contract_id`; si en el día hay dos contratos, la vela es la del contrato de la **última** vela de 1 minuto y las del otro contrato se descartan).
- **Qué es el "cierre":** el precio en la medianoche UTC, entre 1 y 2 horas después del fin de la sesión de CME (17:00 hora de Chicago + la reapertura). **No es** el precio de liquidación oficial. Para una señal que dura semanas la diferencia es chica, pero se dice.
- **Velas de domingo:** se **juntan con la del lunes** (apertura del domingo, cierre del lunes, máximo y mínimo de los dos). Así cada semana tiene 5 cierres y la volatilidad diaria no se mide con un "día" de una hora (punto abierto 6).
- **Días que faltan** (feriados): no se rellenan. Los retornos van de un cierre presente al siguiente.

### 3.4 Vencimientos y serie ajustada

- **Retorno de un día con cambio de contrato:** en las velas UTC, la vela de un día empieza en el mismo instante en que termina la del día anterior. Entonces el retorno del día del cambio se mide **dentro del contrato nuevo**, de su apertura a su cierre, y el salto entre contratos (que no es ganancia ni pérdida) queda afuera. Es exactamente lo que ya hace `research::chained_return` (`engine/crates/backtest/src/research/mod.rs`): `old.close / base`, y la base pasa a `new.open`. Lo que se pierde es solo el movimiento entre la última vela del contrato viejo y la primera del nuevo, que son contiguas (punto abierto 7).
- **Serie de precios para las señales:** ajustada hacia atrás **por diferencia** (método *Panamá*, el que usa Carver): en cada cambio, todo lo anterior se corre por `new.open − old.close`. Las medias móviles y los máximos y mínimos se calculan sobre esa serie. Como la serie ajustada solo usa precios hasta `t`, no mira el futuro: al llegar un vencimiento nuevo cambia el nivel de los precios viejos, pero **no** la diferencia entre medias ni la señal ya calculada (test de no repintado, §11).
- **P/L de la posición:** Δprecio dentro del mismo contrato × multiplicador × contratos (fraccionarios en investigación, §4.6). En el día del cambio, el P/L es del contrato nuevo desde su apertura.
- **Cuándo se renueva:** cuando cambia el `contract_id` de la serie (la regla de Databento: el de más volumen en `v.0`, el más cercano en `c.0`). **No** son las fechas de IG (que vence antes en bonos y materias primas, programa 3 §A.18). Para el escenario de futuros esto es realista; para el de CFD es una aproximación (punto abierto 8).

## 4. H6.1: tendencia estilo Carver

**Hipótesis:** una cartera de tendencia con reglas publicadas, sin ajustar nada, sobre los 15 mercados de §3.2, tiene retorno ajustado por riesgo positivo y alfa positivo frente a comprar y mantener el S&P 500, después de costos (criterio exacto en §7).

Todas las reglas son **funciones puras** de las velas cerradas hasta `t`. La decisión se toma con el cierre de `t` y se ejecuta en `t + 1` (§4.7).

### 4.1 Volatilidad de cada mercado

- `σ_p,i(t)` = desvío exponencial (EWMA, *span* 35 días) de las diferencias diarias de la serie ajustada (en puntos de precio). Lo usan las señales.
- `σ_%,i(t)` = lo mismo sobre los retornos diarios en % (§3.4), anualizado × √256. Lo usa el tamaño.
- **Calentamiento:** sin al menos 35 retornos no hay volatilidad y no hay posición.
- **Piso:** ninguno en la versión recomendada (punto abierto 10; Carver usa en sus versiones nuevas una mezcla con la volatilidad de largo plazo).

### 4.2 Las reglas (parámetros publicados por Carver)

**EWMAC (cruce de medias exponenciales), 3 velocidades:** `(rápida, lenta)` = **(16, 64)**, **(32, 128)** y **(64, 256)** días. Son las tres más lentas de la familia de Carver (2/8 a 64/256): las rápidas pagan más costo y en CFD no tienen sentido.

```text
bruto_EWMAC(r, l) = (EMA_r(P) − EMA_l(P)) / σ_p
pronóstico        = bruto × escalar_EWMAC(r, l)
```

**Ruptura (*breakout*)**, una velocidad: **N = 80 días** (definición de `pysystemtrade`):

```text
máx_N, mín_N = máximo y mínimo de los cierres ajustados de los últimos N días (incluido t)
medio        = (máx_N + mín_N) / 2
bruto        = 40 × (P − medio) / (máx_N − mín_N)       (entre −20 y +20; si máx = mín, 0)
suavizado    = EMA de span N/4 = 20 días del bruto
pronóstico   = suavizado × escalar_ruptura(80)
```

**Escalares de pronóstico** (los que llevan el valor absoluto medio del pronóstico a 10): se usan los **publicados**, no se estiman. Los que recuerda el agente son 3,75 (16/64), 2,65 (32/128) y 1,87 (64/256) para EWMAC (Carver, *Systematic Trading*, 2015) y del orden de 0,7 para la ruptura de 80 días (configuración de ejemplo de `pysystemtrade`). **No están verificados:** al congelar se copian textuales de la fuente, con la cita exacta (punto abierto 11; `CLAUDE.md` §5b, «No inventar»).

**Tope:** cada pronóstico se recorta a **[−20, +20]**.

### 4.3 Pronóstico combinado

```text
combinado = FDM × (¼ EWMAC16 + ¼ EWMAC32 + ¼ EWMAC64 + ¼ Ruptura80), recortado a [−20, +20]
```

- **Pesos iguales** entre las 4 reglas (punto abierto 9; la otra opción es 50 % EWMAC y 50 % ruptura).
- **FDM** (multiplicador por diversificación de pronósticos) **fijo en 1,1**: las cuatro reglas están muy correlacionadas. Estimarlo con los datos sería ajustar algo (punto abierto 12).
- **Calentamiento:** sin las 256 velas de la media lenta y las 80 de la ruptura, el mercado no tiene pronóstico y no tiene posición.

### 4.4 H6.0: el control (sin voto)

- **Momentum de 12 meses por signo** (Moskowitz, Ooi y Pedersen, 2012): el último día hábil de cada mes, con el cierre, `s_i` = signo del retorno encadenado de los últimos **252 días** (§3.4). Si es exactamente 0, no hay posición.
- Pronóstico = **+10 o −10** (el "promedio" de Carver), sin tope adicional.
- **Mismo tamaño, mismos pesos, mismos costos y misma ejecución** que H6.1 (§4.5 a §6). Solo cambia la señal y que se revisa una vez por mes.
- **Para qué está:** si H6.1 no le gana a esta regla de una línea, lo que tenga de bueno viene de "estar en tendencia en muchos mercados", no de sus reglas. Se informa la diferencia mensual H6.1 − H6.0 con su prueba, **sin voto** (punto abierto 4).
- **Ojo, ya visto:** la tendencia de 12 meses **sobre US500, US100 y US30 solos** ya se probó y no pasó (R016). Acá es una pieza de una cartera de 4 clases; igual se dice.

### 4.5 Tamaño: volatilidad objetivo y riesgo por clase

```text
peso_i      = ¼ (por clase) × 1 / (mercados activos de la clase ese día)
nocional_i  = capital × τ × IDM × peso_i × (combinado_i / 10) / σ_%,i
contratos_i = nocional_i / (P_i × multiplicador_i × tipo de cambio a USD)
```

- **Mismo riesgo por clase** (renta variable, tasas, divisas y materias primas): cada clase recibe un cuarto, y dentro de la clase cada mercado activo lo mismo. Así los 5 índices de bolsa, que se mueven casi igual, no pesan cinco veces (autoanálisis D1).
- **τ (volatilidad objetivo anual):** **20 %** (punto abierto 13). El Sharpe no depende de τ (todo escala igual, costos incluidos), salvo por los topes de §4.8. Se informa la volatilidad realizada.
- **IDM** (multiplicador por diversificación de mercados): **fijo en 2,0** (Carver lo limita a 2,5; punto abierto 12). Igual que τ, solo escala.
- **Capital:** fijo en **100.000 USD** de referencia, sin reinvertir (el P/L no cambia el tamaño). Así los retornos mensuales son comparables en todo el período (punto abierto 14).
- **Moneda:** todo en **USD**. Los 15 mercados de CME cotizan en USD (NKD es el Nikkei en dólares, y 6J es yenes en dólares). La conversión a EUR de la cuenta solo entra en el escenario de CFD (§6.2).

### 4.6 Zona sin operar (*buffer*)

- Se cambia la posición solo si `|objetivo − actual| > 10 %` de la posición "media" (la que daría un pronóstico de 10). Entonces se va **hasta el borde de la zona**, no al objetivo (Carver, *position inertia*). Punto abierto 15.
- **Contratos fraccionarios:** en investigación sí (los futuros grandes no dejarían tener 15 mercados con una cuenta chica). Cuántas posiciones serían posibles con contratos enteros, con futuros micro o con el mínimo de IG, se informa (§9), sin voto.

### 4.7 Ejecución

- Señal con el cierre de `t`. **Se opera al cierre de `t + 1`** (la medianoche UTC siguiente), al precio de cierre más los costos (punto abierto 16). La otra opción, la apertura de `t + 1`, cae en el mismo instante que el cierre de `t` en estas velas: sería operar sin demora.
- **Renovación:** con una sola serie continua, el día del cambio solo está el contrato nuevo. Se supone que la posición se renueva en el instante del cambio (cierre del contrato viejo → apertura del nuevo, que son contiguos en estas velas) y se paga su costo (§6).

### 4.8 Riesgo

**Choque con la spec y `CLAUDE.md` §5** (lo mismo que el programa 3, §A.1 a §A.6). La spec opera velas de 15 minutos, 5 posiciones como máximo, con stop estructural de 0,5 a 3 ATR, cierre los viernes y solo índices y oro. Este programa tiene hasta 15 posiciones, de semanas o meses, en 4 clases, y la salida es el pronóstico, no un stop.

- **Recomendación** (punto abierto 17): que la corrida de **investigación** sea el sistema publicado **sin stop por operación** (agregarle uno sería probar otro sistema), con estos límites de cuenta que sí se aplican:
  - apalancamiento por mercado de `config/risk.toml` donde existe (índices 10:1, oro 10:1, otras materias primas 5:1). Para bonos y divisas no hay tope en `config/risk.toml`: se propone **10:1** solo para esta corrida, sin cambiar la configuración;
  - nocional bruto total ≤ **6 × capital** (si se pasa, todas las posiciones se achican por el mismo factor).
- La pérdida máxima diaria del 3 % y el kill switch del 15 % **se informan** (cuántas veces habrían saltado y qué habría pasado) pero **no votan**: con una volatilidad objetivo de 20 %, una caída de 15 % es normal y el kill switch cortaría el sistema justo antes de las rachas buenas. Un stop de desastre como el del programa 3 (1 % del capital por posición) se informa como variante, sin voto.
- **Operarlo en paper** pide un cambio de spec aparte, aprobado por el dueño, que diga cómo se cumple el stop obligatorio de `CLAUDE.md` §5.

## 5. Benchmarks

- **Escenario de futuros:** comprar y mantener **ES** (S&P 500), con un nocional fijo igual al capital, con los mismos vencimientos (§3.4) y los mismos costos (§6.1). Código nuevo pero chico.
- **Escenario de CFD:** comprar y mantener el CFD **US500** de `research::buy_and_hold` (R018), sin cambios.
- **Por qué US500 y no ES+NQ+YM:** el autoanálisis pide no medir contra "el S&P en retorno", sino ver si la cartera agrega algo a quien ya tiene la bolsa de EE. UU. US500 es la referencia más simple; NQ y YM se mueven casi igual (punto abierto 18).

## 6. Costos

### 6.1 Escenario de futuros (principal)

| Costo | Regla | Fuente |
|---|---|---|
| Spread | ½ tick por ejecución (spread de 1 tick) | Tick y multiplicador de cada contrato: especificaciones de CME, leídas y citadas al congelar |
| Slippage | 1 tick por ejecución (como la spec: «0,1 × spread + 1 tick») | spec §11 |
| Comisión | Por contrato y por lado, la de IBKR para el contrato estándar más tasas de bolsa | **PENDIENTE:** en el repo solo está la de los micro (0,25 USD por lado, ADR 0017). El dueño la lee en IBKR |
| Vencimiento | Se cierra el viejo y se abre el nuevo: 2 ejecuciones (spread, slippage y comisión) | — |
| Financiación | Ninguna: el futuro lleva el costo de tener la posición en su precio | — |
| Estrés | Todo × 1,5 (vota) y × 2 (se informa) | — |

### 6.2 Escenario de CFD de IG (segundo, sin voto salvo lo que diga el dueño)

Solo los mercados con CFD de IG y costo conocido. Con lo que hay hoy en `config/instruments.toml`: **US500, US100, US30 y oro** (y DE40, que no entra por FDAX). Para los demás (Russell, Japón 225, bonos, plata, crudo, cobre y divisas) los spreads y la financiación están pendientes de la consulta 5 a IG (`docs/guides/consultas-proveedores.md`, programa 3 §C). Mientras falten, el escenario de CFD cubre solo esos 4 mercados y **no es comparable** con el de futuros.

- **Spread y slippage:** los de `config/instruments.toml` y `costs.rs` (`fill_price`), como el programa 5.
- **Financiación** (pedido del autoanálisis §9, punto 2): cada noche, largo paga **tasa de referencia + 3 %** y corto recibe **tasa − 3 %** (si es negativa, paga), con la tasa de la moneda (SOFR para USD, €STR para EUR) y base 360, menos la base del futuro (#470, ADR 0015): `funding_over` y `basis_carry` de `costs.rs`, sin otra implementación. Las materias primas tienen otra fórmula (ADR 0013).
- **Conversión a EUR:** 0,8 % sobre el P/L en USD (programa 3 §C.1, a confirmar con IG).
- **Recomendación** (punto abierto 19): que el escenario de CFD **se informe y no vote** hasta tener los costos de IG de los 15 mercados. Si pasa en futuros y no en CFD, es la respuesta a "¿el costo del CFD decide?" (autoanálisis §9, punto 2).

## 7. Criterios para pasar (el dueño elige)

Series (mensuales, 153 meses):

- `y` = retorno mensual neto de H6.1 = P/L del mes / capital (escenario de futuros).
- `x` = retorno mensual neto de comprar y mantener ES (§5).
- `y_s`, `x_s` = los mismos con todos los costos × 1,5.

### 7.1 Tres opciones

| Opción | Prueba principal | Ventaja | Problema |
|---|---|---|---|
| **A (recomendada)** | **Alfa** de `y = α + β x + e` > 0, error Newey-West con 3 rezagos, unilateral, p < α₆ | Pide ventaja propia, no beta de la bolsa disfrazada. Alfa > 0 equivale a que **alguna** mezcla con comprar y mantener tenga más Sharpe que comprar y mantener solo. Es la prueba de los programas 3 y 4 | Ninguno grande. Con β ≈ 0 da casi lo mismo que B |
| B | **Sharpe > 0:** t unilateral de la media de `y` (Newey-West, 3 rezagos), p < α₆ | La más simple de leer | Pasa también si la cartera es "largo bolsa" la mayor parte del tiempo |
| C | **Mejora la mezcla:** el Sharpe de 50 % `x` + 50 % `y` (escalada a la volatilidad de `x`) es mayor que el de `x`, con prueba de diferencia de Sharpe (Jobson-Korkie con la corrección de Memmel) | Es exactamente la pregunta "¿me sirve tenerla además del S&P?" | Muy poco poder con 153 meses: casi nunca da significativo |

### 7.2 Criterios con la opción A (todos)

1. **Alfa:** p < **α₆** (§1) en la prueba de la opción A.
2. **Sharpe deflactado ≥ 0,95** de la ventaja `y_t − β x_t`, con el N al congelar (§1). Si la fórmula no da valor, no pasa.
3. **Mitades:** la media de `y_t − β x_t` es **> 0** en 02-2013 → 12-2020 y en 01-2021 → 10-2025.
4. **Costos × 1,5:** el alfa con `y_s` y `x_s` sigue **> 0**.
5. **Agrega a la bolsa:** la cartera `0,5 x + 0,5 y'` tiene un Sharpe mensual **mayor** que `x` sola, donde `y'` es `y` escalada a la volatilidad de `x` en todo el período (sin prueba, solo el signo).
6. **Crisis (§8):** la media de `y` en los meses de crisis es **≥ 0**.
7. **Amplitud:** la contribución al P/L total es **> 0** en al menos **3 de las 4 clases**.

Con la opción B, el criterio 1 cambia por la prueba B y el 2 se aplica a `y`. Con la opción C, el criterio 1 es la prueba C y el 5 desaparece. Los criterios 6 y 7 votan en las tres opciones, salvo que el dueño diga otra cosa (puntos abiertos 20 y 21).

**Pasar quiere decir** "esta cartera tiene ventaja propia y diversifica a quien tiene la bolsa", **no** que le gane al S&P 500 en retorno, ni que se pueda operar con el tamaño de la cuenta (§4.6).

## 8. Comportamiento en las caídas de la bolsa

- **Meses de crisis:** el **10 % de los meses** con peor retorno de comprar y mantener ES dentro de los 153 (15 meses; si hay empate en el borde, entran todos los empatados). Se define con `x` antes de mirar `y`.
- **Criterio 6 (vota en la recomendación):** media de `y` en esos meses **≥ 0**. Sin prueba de significancia: con 15 meses no tendría poder, y la pregunta es si la cartera **no se hunde con la bolsa**.
- **Descriptivo:** `y` en cada uno de esos 15 meses; la correlación entre `y` y `x` en los meses de `x` < 0 y en los de `x` > 0; el retorno de `y` en las 5 mayores caídas de ES (de pico a valle, con fechas), y la mayor caída de `y` y su duración.

## 9. Descriptivo (sin voto)

- Por mercado y por clase: retorno, volatilidad realizada, Sharpe, contribución al P/L, costo pagado (operaciones y vencimientos), cantidad de operaciones y de vencimientos, tiempo medio con posición, % del tiempo largo, corto y sin posición.
- Por regla (las 3 EWMAC y la ruptura, cada una sola con el mismo tamaño): Sharpe y correlación entre ellas. **No votan** y no se elige ninguna después (§13).
- Cartera: volatilidad realizada contra τ, apalancamiento bruto (medio y máximo), veces que actuó el tope de §4.8, costo anual en % y en "R" (costo / riesgo de la posición), H6.1 − H6.0 por mes con su t.
- **Factibilidad:** con contratos enteros estándar, con futuros micro (donde existen) y con los mínimos de IG, cuántas de las posiciones objetivo se podrían abrir con 10.000 €, 100.000 € y 1.000.000 €.
- Pérdida diaria del 3 % y kill switch del 15 %: cuántas veces habrían saltado (§4.8).
- Escenario de CFD (§6.2), costos × 2, DSR con N = 100 y N = 1000.
- Monte Carlo por bloques de las caídas y riesgo de ruina (#662) y PBO (#664), si ya están en `research::stats` al correr.
- Todo lo de arriba también para H6.0.

## 10. Test de suerte (solo reporte)

- **Generador:** SplitMix64, semilla 20261125, un solo flujo en orden fijo (mercados en el orden de §3.2, días en orden).
- **Modelo nulo de Tom Basso** (autoanálisis C7): **1.000 carteras** con el mismo tamaño, los mismos costos y la misma zona sin operar, pero con el **signo** del pronóstico de cada mercado al azar, cambiando con la misma frecuencia media que H6.1 en ese mercado (duraciones geométricas). El valor absoluto del pronóstico se mantiene.
- Se informa en qué percentil de esas 1.000 cae el Sharpe de H6.1 (y el de H6.0).

## 11. Poder: cuánto Sharpe hace falta

Con 153 meses (12,75 años), el estadístico t de la media es aproximadamente **Sharpe anual × √12,75 ≈ Sharpe anual × 3,57**. El Sharpe anual (después de costos) que daría **justo** el umbral (poder de 50 %):

| α₆ | z unilateral | Sharpe anual necesario |
|---|---|---|
| 0,05 | 1,645 | 0,46 |
| 0,025 | 1,960 | 0,55 |
| 0,0125 | 2,241 | 0,63 |
| 0,01 | 2,326 | 0,65 |
| 0,005 | 2,576 | 0,72 |

Es una cuenta, no un resultado. Dice que con un α₆ chico (como el que puede salir de un presupuesto global, #660) **una cartera de tendencia buena pero normal puede no pasar**. Es otra razón para dar a este programa una parte grande del presupuesto que quede, y no repartirla con otras pruebas en estos datos.

## 12. Código que hace falta (al congelar)

**Se reutiliza sin cambios** (no se copia ninguna definición):

- `research::stats`: `newey_west_t_greater`, `alpha_hc1` (y su versión con Newey-West, ver abajo), `deflated_sharpe`, `t_test_greater`, `skew_kurtosis`.
- `research::chained_return` para los retornos en los vencimientos.
- `costs.rs` (`fill_price`, `commission`, `funding_over`, `basis_carry`) para el escenario de CFD, y `research::buy_and_hold` para el benchmark de CFD.
- El SplitMix64 del programa 5.

**Nuevo:**

1. **Lectura de velas diarias** en el motor: hoy el motor no lee `futures_bars_1d` ni `config/databento_daily.toml`. Hace falta leer las 10 diarias y armar las diarias UTC de las 6 de 1 minuto (§3.3), con el corte `< 2025-11-01`, el `contract_id` y la unión domingo-lunes.
2. **`research::program6`** (funciones puras con tests): serie ajustada por diferencia, EWMA de volatilidad, EWMAC, ruptura, escalares, tope, combinación, tamaño por clase, zona sin operar, ejecución en `t + 1`, P/L por contrato y costos de futuros con vencimientos.
3. **Costos de futuros:** hoy `costs.rs` es de CFD. Hace falta el modelo de §6.1 (tick, multiplicador, comisión por contrato) con sus datos en configuración.
4. **Alfa con Newey-West:** `alpha_hc1` usa HC1. Para la opción A hace falta la variante con Newey-West de 3 rezagos (como `newey_west_t_greater`), con tests contra valores calculados a mano.
5. **Benchmark de ES** con vencimientos (§5) y el test de suerte de §10.
6. **Configuración** (al congelar, no ahora): `[rule_program6]` y `[program6]` en `config/research.toml`, y las especificaciones de los 15 contratos (tick, multiplicador, moneda, clase, comisión) en un archivo nuevo de `config/` validado al arrancar.
7. **Ejemplo `programa_6`** en `engine/crates/backtest/examples/`, que imprime primero los conteos (velas, vencimientos, mercados activos por mes) y después los resultados.

**Tests obligatorios:** no mirar el futuro (cambiar velas posteriores a `t` no cambia la señal de `t`), no repintar (la señal de una vela cerrada no cambia con datos nuevos, incluido un vencimiento nuevo), calentamiento, vencimientos (incluidos los que vuelven a un contrato anterior), días faltantes, máx = mín en la ruptura, tope ±20, zona sin operar, topes de apalancamiento y el orden de la ejecución.

Es más de 400 líneas: se propone en **dos PRs** después de congelar (datos y reglas; corrida e informe).

## 13. Prohibido después de ver resultados

Todo esto cuenta como una prueba nueva, con su costo en el presupuesto:

- cambiar las velocidades, la ruptura, los escalares, el tope, los pesos de las reglas, el FDM, el *span* de la volatilidad, el piso, τ, el IDM, los pesos por clase, la zona sin operar, el día de ejecución o los topes;
- sacar mercados o clases que pierden, o quedarse con la regla que dio mejor (las de §9 son descriptivas);
- cambiar los costos, el escenario que vota, el benchmark, la definición de los meses de crisis, el corte de las mitades, el nivel o el N;
- informar solo el escenario, la mitad o la clase más favorable;
- mirar el fuera de muestra sin la aprobación del dueño;
- volver a correr con otra regla y presentar la mejor.

**Errores de código:** se corrigen, se vuelve a correr y se informa con fecha: cuál fue el error, el resultado anterior y el nuevo. El anterior no se borra (fila propia en el registro, con `compara_con`).

## 14. Al congelar y al correr

- **Congelar:** con las decisiones del dueño en una sección nueva («Decisiones del dueño», con sus palabras y la fecha, citadas en #672), se quita "borrador", se verifican y citan los escalares de §4.2, se commitea con fecha y hora y se agrega `2026-11-programa-6-tendencia-preregistro.md.sha256`. En el mismo PR, `[rule_program6]` y `[program6]` en `config/research.toml`. `config/risk.toml`, `config/strategy.toml`, el runner y el paper **no cambian**.
- **Correr:** cada corrida se anota con su hora (UTC) y su comando, y agrega su fila en `docs/analysis/registro-de-resultados.csv` en el mismo PR que su informe.
- **Si pasa:** se informa al dueño; él decide si se mira el fuera de muestra (una vez) y si se abre un Issue de spec para operarlo en paper. Pasar no habilita operar ni dinero real.
- **Si no pasa:** la hipótesis se cierra. No se prueban variantes sobre estos datos.

## 15. Puntos abiertos para el dueño

Todos **PENDIENTES**. Ningún agente los decide. Cada respuesta queda en #672 con las palabras del dueño y la fecha.

| # | Punto | Valor recomendado | Por qué | Estado |
|---|---|---|---|---|
| 1 | Relación con el programa 3 (#513) | El programa 6 **reemplaza** al 3; el 3 se cierra sin congelar y sus anexos se reutilizan | Son dos pruebas casi iguales sobre los mismos datos: correr las dos gasta presupuesto dos veces e invita a quedarse con la mejor | PENDIENTE |
| 2 | Presupuesto (α₆) | El que dé la regla global de #660; si se congela antes, 5 % nuevo con **α₆ = 0,025** para H6.1 y 2,5 % en reserva | No duplicar #660. Con un α₆ más chico el poder cae mucho (§11) | **Resuelto por #660 (aprobada el 25-11-2026, #690):** α₆ sale del libro global, ≤ 1,25 % con `W` = 4,75 %; la opción de 5 % nuevo ya no existe (nota de §1) |
| 3 | N del Sharpe deflactado | N del registro al congelar + 2 (hoy 361 → **363**) | Se suman H6.1 y H6.0, aunque H6.0 no vote, porque igual se mira | PENDIENTE |
| 4 | Papel de H6.0 | **Control sin voto**; se informa H6.1 − H6.0 | Dos hipótesis de tendencia que votan sobre los mismos datos parten el presupuesto y casi dan lo mismo | PENDIENTE |
| 5 | FDAX | **Fuera** del universo (se dice en el informe) | Solo tiene datos desde el 11-03-2025: menos de 8 meses antes del fuera de muestra, menos que el calentamiento | PENDIENTE |
| 6 | Velas de domingo | **Juntarlas con el lunes** | Son una hora de sesión; contarlas como un día achica la volatilidad medida y mete un "día" falso por semana | PENDIENTE |
| 7 | Retorno del día de vencimiento | **Dentro del contrato nuevo**, de apertura a cierre (`chained_return`) | Las velas UTC son contiguas: se pierde casi nada y el salto entre contratos no se cuenta como P/L. Bajar `v.1` (≈ USD 0,5) para tener los dos contratos ese día es la otra opción | PENDIENTE |
| 8 | Fechas de vencimiento | Las de la serie de Databento (`contract_id`), no las de IG | No hay calendario de IG desde 2011. Para el escenario de futuros es lo real | PENDIENTE |
| 9 | Pesos de las reglas | **¼ cada una** (3 EWMAC + ruptura 80) | Lo más simple y sin elegir; 50/50 EWMAC/ruptura daría más peso a una sola regla de ruptura | PENDIENTE |
| 10 | Volatilidad | EWMA *span* 35, **sin piso** | Es la de Carver en sus libros; el piso con la volatilidad larga es una mejora posterior que agrega un parámetro | PENDIENTE |
| 11 | Escalares de pronóstico | Los **publicados**, copiados con cita al congelar (no estimados) | Estimarlos con estos datos sería ajustar; los valores de §4.2 los escribió el agente de memoria y hay que verificarlos | PENDIENTE |
| 12 | FDM e IDM | **Fijos: FDM 1,1 e IDM 2,0** | Solo escalan la cartera (no cambian el Sharpe salvo por los topes); fijos evitan estimar nada | PENDIENTE |
| 13 | Volatilidad objetivo τ | **20 % anual** | No cambia el Sharpe; 20 % es un valor medio de los que publica Carver y deja lejos los topes de apalancamiento | PENDIENTE |
| 14 | Capital | **100.000 USD fijo, sin reinvertir** | Retornos comparables en todo el período; con contratos fraccionarios el tamaño no cambia el resultado | PENDIENTE |
| 15 | Zona sin operar | **10 %** de la posición media, ir al borde | Es la regla de Carver; baja el costo sin cambiar la señal | PENDIENTE |
| 16 | Ejecución | **Cierre de `t + 1`** | La apertura de `t + 1` es el mismo instante que el cierre de `t` en estas velas: sería operar sin demora | PENDIENTE |
| 17 | Stop y límites de cuenta | **Sin stop por operación** en la investigación; topes de apalancamiento (bonos y divisas 10:1 solo acá) y bruto ≤ 6 × capital; 3 % diario y kill switch 15 % solo se informan | El sistema publicado sale por la señal; un stop o el kill switch lo cambiarían. Operarlo pide cambio de spec | PENDIENTE |
| 18 | Benchmark | **ES** en futuros y **US500** en CFD | Es "quien ya tiene la bolsa de EE. UU."; NQ y YM se mueven casi igual | PENDIENTE |
| 19 | Escenario de CFD | **Se informa, no vota**, hasta tener los costos de IG de los 15 mercados | Hoy solo hay costos de IG de 4 mercados; votar con eso mediría otra cartera | PENDIENTE |
| 20 | Criterio principal | **Opción A (alfa contra ES)** con los 7 criterios de §7.2 | Pide ventaja propia y equivale a que alguna mezcla con la bolsa mejore; B pasaría con beta disfrazada y C casi no tiene poder | PENDIENTE |
| 21 | Crisis y amplitud | **Votan** (criterios 6 y 7), sin prueba de significancia | Son la razón económica del programa: si la cartera se hunde con la bolsa o vive de una sola clase, no sirve para lo que se quiere | PENDIENTE |
| 22 | Comisión de futuros estándar | La que el dueño lea en IBKR, con fecha | En el repo solo está la de los micro (ADR 0017); no se inventa | PENDIENTE |
| 23 | Especificaciones de los contratos | Tick, multiplicador y moneda de las páginas de CME, citadas | Hacen falta para el P/L y los costos; no se inventan | PENDIENTE |
| 24 | Choque con la spec (programa 3 §D.3) | El programa queda **solo de investigación**; operarlo pide un cambio de spec aparte | La spec es de 15 minutos, 5 posiciones, índices y oro | PENDIENTE |
| 25 | Riesgo total (programa 3 §D.10) | Lo controla la volatilidad objetivo, no un 1 % por posición | Con 15 posiciones, la regla de 1 % por operación no aplica a este tipo de sistema; se informa la pérdida en el peor día | PENDIENTE |
| 26 | Consulta a IG (programa 3 §C, consulta 5) | Mandarla (solo el dueño) si se quiere que el escenario de CFD vote algún día | Sin los spreads y la financiación de los 15 mercados, el escenario de CFD queda incompleto | PENDIENTE |

## Fuentes

- Interno: `docs/analysis/2026-11-autoanalisis.md` (§3, §4, §9 y §10), `docs/analysis/2026-11-programa-3-borrador.md` (anexos A a D), `docs/analysis/2026-11-programa-4-preregistro.md` (§7), `docs/analysis/2026-11-programa-5-fvg-preregistro.md`, `config/databento_daily.toml`, `config/databento.toml`, `config/data_split.toml`, `config/research.toml`, `config/instruments.toml`, `config/risk.toml`, `docs/decisions/0013-materias-primas-oro.md`, `docs/decisions/0015-tasas-usd-nyfed-y-base-de-futuros.md`, `docs/decisions/0017-ibkr-paper-futuros-micro.md`, `docs/analysis/registro-de-resultados.csv` (R016, R018 y R019), `engine/crates/backtest/src/research/` (`mod.rs`, `stats.rs`, `buy_and_hold.rs`, `trend.rs`).
- [Moskowitz, Ooi y Pedersen 2012, *Time series momentum*](https://doi.org/10.1016/j.jfineco.2011.11.003) · [Hurst, Ooi y Pedersen, AQR, *A Century of Evidence on Trend-Following Investing*](https://www.aqr.com/Insights/Research/Journal-Article/A-Century-of-Evidence-on-Trend-Following-Investing) · Robert Carver, *Systematic Trading* (2015) y *Leveraged Trading* (2019) · [pysystemtrade](https://github.com/pst-group/pysystemtrade) · Jobson y Korkie (1981) con la corrección de Memmel (2003), prueba de diferencia de Sharpe.
