# 1.3 Apalancamiento y margen

> **Nivel 1 · Instrumentos.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Entender el [margen](../GLOSARIO.md#margen) minorista, nuestro **tope propio** de
[apalancamiento](../GLOSARIO.md#apalancamiento) y el [cierre por margen](../GLOSARIO.md#stop-out).

## La idea

**Nominal.** Es lo que "mueve" tu posición: tamaño × precio. Un CFD del US100 de tamaño 4 en
25.000 (precio de ejemplo) tiene un nominal de 4 × 25.000 = 100.000 €.

**Margen.** Es la garantía que el broker te bloquea para abrir la posición. La regla europea fija un
mínimo por tipo de activo; para los índices principales (S&P 500, NASDAQ 100, Dow Jones, DAX) es el
**5 % del nominal**. IG pide ese 5 % en nuestros cuatro índices.

| Activo (ESMA) | Margen mínimo | Apalancamiento máximo |
|---|---|---|
| Pares de divisas principales | 3,33 % | 30:1 |
| Índices principales, otras divisas y oro | 5 % | 20:1 |
| Otros índices y materias primas | 10 % | 10:1 |
| Acciones | 20 % | 5:1 |
| Criptomonedas | 50 % | 2:1 |

**Apalancamiento.** Es nominal ÷ capital. Con 5 % de margen podrías llegar a 20:1. **Nosotros no
pasamos de 10:1** en índices: es un tope propio, la mitad del máximo permitido. El apalancamiento
multiplica igual las ganancias y las pérdidas: con 10:1, una caída del 1 % del índice es un 10 % de
tu capital.

**Cierre por margen.** Si tu capital (lo que hay en la cuenta más lo que ganan o pierden las
posiciones abiertas) baja de **la mitad del margen inicial**, el broker cierra las posiciones. Es
una protección de la regla europea, no una decisión tuya.

**Cómo decide el tamaño nuestro motor.** Primero el riesgo: arriesgás el **1 %** del capital entre
la entrada y el stop (más un colchón de 0,25 ATR por los gaps). Recién después mira el tope: si ese
tamaño pasa 10:1, lo achica. El tope solo actúa cuando el stop es muy corto (ejemplo abajo).

> **Probá antes de seguir:** con 10.000 € y 20:1, ¿cuánto margen libre te queda después de abrir el
> máximo?
>
> <details><summary>Ver la respuesta</summary>
>
> Nada: el nominal sería 200.000 € y el margen del 5 % son 10.000 €, todo tu capital. Cualquier
> pérdida te acerca al cierre por margen.
>
> </details>

## Interactivo

Una calculadora: elegís instrumento, tamaño y capital, y el motor devuelve nominal, margen y
apalancamiento efectivo (el mismo veredicto del ticket, `ticket_verdict.v1`). La UI no calcula.

## Ejemplo resuelto

Capital 10.000 €, US100 en 25.000 (precio de ejemplo), 1 € por punto.

1. **Tamaño máximo con el tope de 10:1:** 10 × 10.000 ÷ 25.000 = **4 € por punto** (nominal
   100.000 €).
2. **Margen que bloquea el broker:** 5 % × 100.000 = **5.000 €**.
3. **Cierre por margen:** la mitad del margen inicial, 2.500 €. Te cierran si perdés 7.500 €:
   7.500 ÷ 4 = **1.875 puntos**, un 7,5 % de caída del índice.
4. **Lo mismo con 20:1:** tamaño 8, margen 10.000 €, cierre a 5.000 €. Alcanza con perder 5.000 €:
   5.000 ÷ 8 = **625 puntos**, un 2,5 % de caída.

Con la mitad de apalancamiento, el mercado tiene que moverse tres veces más para sacarte. Ojo: el
broker puede recalcular el margen con el precio del momento, así que el punto exacto de cierre
puede correrse un poco; acá usamos el margen de la entrada.

**Cuándo manda el tope.** Riesgo 1 % = 100 €. Con un ATR de 30 puntos y el stop más corto
permitido (0,5 ATR = 15 puntos), la distancia con el colchón es 15 + 0,25 × 30 = 22,5 puntos, y el
tamaño por riesgo sería 100 ÷ 22,5 ≈ 4,44. Eso pasa el tope de 4, así que el motor lo baja a 4 y el
riesgo queda en 4 × 22,5 = 90 € (0,9 %): **por debajo** del 1 %, nunca por encima.

## Ejercicio

Con un capital de 5.000 €, ¿cuál es el tamaño máximo que permite el tope de 10:1 en US100 a
25.000? ¿Y cuánto margen pide IG para ese tamaño?

<details><summary>Ver la solución</summary>

10 × 5.000 ÷ 25.000 = **2 € por punto** (nominal 50.000 €). Margen: 5 % × 50.000 = **2.500 €**.

</details>

## Examen

8 preguntas (4 de concepto y 4 de cálculo). Para aprobar hace falta el 80 % (umbral
provisorio).

## Desbloquea

Nada nuevo: la calculadora de tamaño llega con el ticket de paper (nivel 7).

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Margen minorista de IG en US100 | 5 % (`5`) | `config/instruments.toml` `instruments.US100.margin_retail_pct` (ig.com/es/indices) | A |
| Margen minorista de IG en US500 | 5 % (`5`) | `config/instruments.toml` `instruments.US500.margin_retail_pct` | A |
| Margen minorista de IG en US30 | 5 % (`5`) | `config/instruments.toml` `instruments.US30.margin_retail_pct` | A |
| Margen minorista de IG en DE40 | 5 % (`5`) | `config/instruments.toml` `instruments.DE40.margin_retail_pct` | A |
| Tope propio de apalancamiento en US100 | 10:1 (`10`) | `config/instruments.toml` `instruments.US100.max_leverage` (spec §8) | A |
| Tope propio de apalancamiento en DE40 | 10:1 (`10`) | `config/instruments.toml` `instruments.DE40.max_leverage` | A |
| Cierre por margen | 50 % (`50`) | `config/instruments.toml` `account.margin_closeout_pct` (decisión 2018/796 de ESMA) | A |
| Riesgo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Colchón por gaps | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
| Stop más corto permitido | 0,5 ATR (`0.5`) | `config/risk.toml` `stop_distance.min_atr` | A |
| Tamaño = tope × capital ÷ entrada si se pasa del tope | `tamaño = tope × capital_actual / entrada` | `docs/spec/fase-1-especificacion.md` §8 | A |
| Márgenes mínimos por activo (3,33 %, 5 %, 10 %, 20 %, 50 %) y cierre al 50 % | — | [ESMA, aviso ESMA35-43-1135 (2018), artículo 1(e) y anexo I](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf), leído el 26-11-2026 | A |
| Las mismas reglas en España | — | [Resolución de la CNMV de 27-06-2019 (BOE-A-2019-9737), anexo I](https://www.boe.es/buscar/act.php?id=BOE-A-2019-9737), leída el 26-11-2026 | A |
