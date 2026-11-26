# 1.1 Contado, futuros y CFD

> **Nivel 1 · Instrumentos.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa
> en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Saber qué tenés en la mano en cada caso: **el activo** (contado), **un contrato con vencimiento**
(futuro) o **una diferencia de precio con un broker** ([CFD](../GLOSARIO.md#cfd)).

## La idea

| | Contado | Futuro | CFD al contado (IG) |
|---|---|---|---|
| Qué tenés | El activo (por ejemplo, una acción) | Un contrato estándar de una bolsa | Un contrato con el broker |
| ¿Vence? | No | Sí, en una fecha fija | No: lo cerrás cuando quieras |
| Con quién | Con otro inversor, por la bolsa | Con otro inversor, por la bolsa (cámara de compensación) | Con el proveedor del CFD |
| Tamaño | Lo que compres | Fijo por contrato | Flexible, desde un mínimo |
| Costo de mantenerlo | Ninguno extra | Va incluido en el precio del futuro | Financiación cada noche (nivel 2) |

**El futuro del S&P 500 (ES).** Es el que usamos para estudiar el pasado: tenemos sus velas de 1
minuto desde 2011. Cada contrato vale **50 USD por punto**: si el índice sube 4 puntos, un contrato
gana 200 USD. El precio se mueve de a 0,25 puntos (un "tick"), que valen 12,50 USD.

El ES **vence** cuatro veces por año: marzo, junio, septiembre y diciembre (letras H, M, U y Z).
El nombre de cada contrato lleva la letra del mes y el último dígito del año: el de diciembre de
2026 es **ESZ6**. Para seguir posicionado hay que pasar al contrato siguiente antes de que venza:
eso se llama **roll**. Cada vencimiento tiene su
propio precio (el futuro descuenta intereses y dividendos hasta su fecha), así que la serie
continua del futuro **salta** el día del cambio. Ese salto no es un movimiento del mercado: el
motor no arma una vela que mezcle dos contratos y no cuenta el salto como retorno.

**El CFD del US 500.** Es el que se opera (en paper). No vence, vale **1 € por punto** y el tamaño
mínimo es 1. Su precio sigue al índice de contado, así que no tiene el salto del roll. A cambio,
cada noche que lo mantenés abierto pagás (o cobrás) financiación.

> **Probá antes de seguir:** ¿por qué usamos el futuro para estudiar el pasado y el CFD para operar?
>
> <details><summary>Ver la respuesta</summary>
>
> Del futuro tenemos casi 15 años de velas de 1 minuto. Del CFD de IG casi no hay historia guardada
> antes del período reservado. Por eso se estudia con el futuro y se cobran los costos del CFD
> (spread, slippage, financiación) como si fuera el CFD.
>
> </details>

## Interactivo

El mismo día en el CFD US500 (IG) y en el futuro ES (Databento), lado a lado: precio, vencimiento
y la marca del roll. El **selector de fuente** cambia el gráfico entre CFD y futuros.

## Ejemplo resuelto

Un ES sube de 6.000 a 6.010 (precios de ejemplo): 10 puntos × 50 USD = **500 USD por contrato**.
El CFD del US 500 con tamaño 1 hace el mismo recorrido: 10 puntos × 1 € = **10 €**. Mismo mercado,
tamaños muy distintos: con el CFD se puede operar algo chico.

## Ejercicio

En el gráfico del futuro, encontrá el último roll de septiembre a diciembre y medí el salto en
puntos. Después explicá con una frase por qué el CFD no tiene ese salto.

<details><summary>Ver la solución</summary>

El salto es la diferencia de precio entre el contrato que vence y el siguiente, ese día. El CFD
al contado no cambia de contrato, así que su serie no salta; su "costo de mantenerlo" aparece cada
noche como financiación.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Selector de fuente** (CFD / futuros) en los gráficos.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| ES: USD por punto | 50 (`50`) | `config/futures_contracts.toml` `contracts.ES.multiplier` (IBKR y página de CME, 25-11-2026) | A |
| ES: tick | 0,25 puntos (`0.25`) | `config/futures_contracts.toml` `contracts.ES.tick_size` | A |
| ES: valor del tick | 12,50 USD (`12.50`) | `config/futures_contracts.toml` `contracts.ES.tick_value` | A |
| ES: meses de vencimiento | `HMUZ` | `config/futures_contracts.toml` `contracts.ES.listed_months` | A |
| Letras de los meses (H marzo, M junio, U septiembre, Z diciembre) | `Z dic` | `config/futures_contracts.toml` (encabezado: letras de CME) | A |
| Velas de 1 minuto de futuros desde | `2011-01-01` | `config/databento.toml` `download.start` | A |
| CFD US500: € por punto | 1 (`1`) | `config/instruments.toml` `instruments.US500.value_per_point` (API de IG, demo) | A |
| CFD US500: tamaño mínimo | 1 (`1`) | `config/instruments.toml` `instruments.US500.min_deal_size` | A |
| El replay del examen usa velas de futuros | `futures` | `config/academy.toml` `replay.source` | A |
| Especificación oficial del ES | — | [CME Group, E-mini S&P 500, especificaciones](https://www.cmegroup.com/markets/equities/sp/e-mini-sandp500.contractSpecs.html) (comparada con la config el 25-11-2026) | A |
| Precio del futuro: intereses y dividendos hasta el vencimiento | — | [Hull, *Options, Futures, and Other Derivatives*, 11.ª ed., Pearson, 2021](https://www-2.rotman.utoronto.ca/~hull/ofodnew/index.html) | B |
| Contado, futuros y cámaras de compensación | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
