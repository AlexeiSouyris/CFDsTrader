# 0.1 Qué es un mercado

> **Nivel 0 · Orientación.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Entender que del otro lado de cada operación hay alguien, y que el precio que ves es **el último
acuerdo**, no una promesa de lo que va a pasar.

## La idea

Un mercado es un lugar donde unos quieren comprar y otros quieren vender. Hay operación solo cuando
**los dos aceptan el mismo precio**. Ese precio queda como el "último": es lo que pagó alguien hace
un instante, nada más.

- Si muchos quieren comprar y pocos vender, los compradores tienen que ofrecer más: el precio sube.
- Si pasa al revés, el precio baja.
- Nadie sabe cuál va a ser el próximo acuerdo. Por eso el precio de ahora **no dice** cuál va a ser
  el de mañana.

En la pantalla vas a ver siempre **dos precios**:

| Botón | Qué es | Quién lo usa |
|---|---|---|
| **BUY** (compra) | el precio al que te venden | vos, si querés comprar |
| **SELL** (venta) | el precio al que te compran | vos, si querés vender |

El BUY siempre es un poco más alto que el SELL. Esa diferencia es el
[spread](../GLOSARIO.md#spread), y lo vas a estudiar en el nivel 2. En el US 500, nuestro spread
típico es de 0,9 puntos.

**¿Y con un CFD?** Un [CFD](../GLOSARIO.md#cfd) no se compra en la bolsa: es un contrato con el
broker, que la regla europea llama "proveedor de CFD". Nosotros usamos IG (en simulación). IG
cotiza su CFD siguiendo al índice de verdad, y el motor tiene una verificación que compara los
cierres diarios del CFD con los del futuro del mismo índice.

**Horarios.** La bolsa de Nueva York abre de 09:30 a 16:00, hora de Nueva York. El CFD de IG, en
cambio, tiene precio casi todo el día: en la demo medimos spreads también entre las 00:00 y las
06:00 UTC, más anchos que en la sesión. El motor guarda todas las horas en **UTC** (con su zona
horaria), así no se mezclan los cambios de horario.

> **Probá antes de seguir:** si nadie quiere vender a 6.000, ¿podés comprar a 6.000?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Si nadie acepta vender a ese precio, no hay acuerdo. Para comprar ya, tenés que pagar el
> precio al que alguien sí vende (el BUY).
>
> </details>

## Interactivo

La **lista de mercados** (watchlist) en solo lectura, con el último cierre que manda el motor para
cada instrumento: último precio, variación y hora. Tocá un instrumento para abrir su panel.

## Ejemplo resuelto

El cierre de ayer del US 500 fue 6.000 y el último precio de hoy es 6.030 (precios de ejemplo,
redondos, no son cotizaciones).

- Variación en puntos: 6.030 − 6.000 = **+30 puntos**.
- Variación en %: 30 ÷ 6.000 × 100 = **+0,5 %**.

Eso dice lo que **ya pasó**. No dice nada de lo que va a pasar.

## Ejercicio

Elegí 3 instrumentos de la lista y anotá, para cada uno: el último precio, la variación del día
y la hora de ese último precio. Después contestá: ¿la hora está en UTC o en la hora de la bolsa?

<details><summary>Ver la solución</summary>

El motor manda la hora en UTC con su zona horaria. Para pasarla a la hora de Nueva York, en
invierno se restan 5 horas (Nueva York está en UTC−5 en noviembre).

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio hasta que el dueño lo apruebe).

## Desbloquea

- **Watchlist** (lista de mercados).
- **Panel del instrumento**.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Spread típico del US 500 | 0,9 puntos (`0.9`) | `config/instruments.toml` `instruments.US500.spread_typical_points` (IG, medido en la demo, #67) | A |
| Apertura de la bolsa de Nueva York | 09:30 (`09:30`) | `config/sessions.toml` `sessions.XNYS.open` | A |
| Cierre de la bolsa de Nueva York | 16:00 (`16:00`) | `config/sessions.toml` `sessions.XNYS.close` | A |
| Zona horaria de la bolsa de Nueva York | `America/New_York` | `config/sessions.toml` `sessions.XNYS.timezone` | A |
| Spread del CFD medido de 00:00 a 06:00 UTC | franja `00–06` | `docs/research/ig-spreads-2026-11.md` (mediana por hora, demo de IG, 09 al 20-11-2026) | A |
| Nueva York en UTC−5 en noviembre | `UTC−5` | `docs/research/ig-spreads-2026-11.md` | A |
| Umbral para aprobar un examen (provisorio) | 80 % (`80`) | `config/academy.toml` `thresholds.quiz_pass_pct` | A |
| Definición de CFD y de "proveedor de CFD" | — | [ESMA, aviso de las decisiones sobre CFD (ESMA35-43-1135, 2018), artículo 1](https://www.esma.europa.eu/sites/default/files/library/esma35-43-1135_notice_of_pi_decisions_on_cfds_and_binary_options.pdf), leído el 26-11-2026 | A |
| Cómo funciona un mercado (órdenes, compradores y vendedores) | — | [Harris, *Trading and Exchanges: Market Microstructure for Practitioners*, Oxford University Press (fechado 24-10-2002 por la editorial; se cita como 2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
