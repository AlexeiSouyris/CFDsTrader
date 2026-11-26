# 7.3 Tamaño

> **Nivel 7 · Riesgo y tamaño.** Educativo: no es consejo de inversión ni promete ganancias. Todo
> pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos
> son redondos, no cotizaciones.

## Objetivo

Calcular el tamaño de una posición como lo calcula el motor: **1 % de riesgo**, el colchón por
gaps, el tope del grupo correlacionado de EE. UU., el tope de
[apalancamiento](../GLOSARIO.md#apalancamiento) de 10:1 y el mínimo del broker.

## La idea

El tamaño **no se elige**: sale del riesgo. Lo calcula el módulo de riesgo del motor, el único que
da tamaños.

**1. El tamaño base.**

tamaño (€ por punto) = riesgo en € ÷ (distancia al stop + 0,25 × ATR)

- **Riesgo en €** = 1 % del capital actual (menos si la cuenta está en caída, lección 7.4).
- **0,25 × ATR** es el **colchón por gaps**: el tamaño se calcula como si el stop estuviera un poco
  más lejos, así un salto por encima del stop también entra en el 1 %.

Capital 10.000 €, US500 en 6.000, ATR 20 y stop a 30 puntos: 100 ÷ (30 + 5) = **2,86 € por punto**.
Si el precio toca el stop justo, perdés 2,86 × 30 ≈ 85,7 €; si salta 5 puntos más, 100 €.

**2. El tope del grupo correlacionado.** US500, US100, US30 y US2000 se mueven casi igual: abrir
los cuatro al 1 % sería casi una sola apuesta del 4 %. Por eso el riesgo abierto de **todo el
grupo** (las posiciones abiertas más la nueva) no puede pasar del **1,5 %** del capital.

- Si queda lugar parcial, el tamaño **se achica** hasta llenarlo.
- Si no queda lugar, se **rechaza** ("tope del grupo").
- DE40 y el oro no tienen pareja en el catálogo: no están en ningún grupo.

**3. El tope de apalancamiento.** El nocional (tamaño × precio) no puede pasar de **10 veces** el
capital: tamaño máximo = 10 × capital ÷ precio. Si el tamaño por riesgo es mayor, se achica al
tope, y entonces el riesgo queda **por debajo** del 1 %.

**4. El margen y el mínimo del broker.** Si el margen no alcanza, se rechaza. Si el tamaño queda
por debajo del mínimo de IG (1 € por punto en US500, 0,5 en US100), también se rechaza: el motor
no lo sube hasta el mínimo, porque eso arriesgaría más del 1 %.

**Y antes de todo eso, los límites de la cuenta**, en este orden: kill switch, caída máxima,
pérdida del día (lección 7.4), **máximo 5 posiciones abiertas**, **una por instrumento**, y la
distancia al stop (lección 7.2). Recién después se mira el grupo y se calcula el tamaño.

> **Probá antes de seguir:** capital 10.000 €, US100 en 25.000, ATR 20 y stop a 10 puntos (0,5
> ATR). ¿Qué tamaño da el motor?
>
> <details><summary>Ver la respuesta</summary>
>
> Por riesgo: 100 ÷ (10 + 5) = 6,67 € por punto. Tope de apalancamiento: 10 × 10.000 ÷ 25.000 =
> **4 € por punto**. Manda el tope: 4. El riesgo queda en 4 × 15 = 60 €, un 0,6 % (menos del 1 %).
>
> </details>

## Interactivo

La calculadora de tamaño del ticket (`POST /api/v1/paper/preview`, ya existe): entrada, stop y
ATR, y el motor devuelve el tamaño, el margen, el apalancamiento, si lo achicó un tope y por qué.

## Ejemplo resuelto

Capital 10.000 €. Ya hay un largo abierto en US100 que arriesga 100 €. Llega una señal en US500 en
6.000, ATR 20, stop a 30 puntos.

- Lugar en el grupo: 1,5 % × 10.000 − 100 = **50 €**.
- Tamaño por riesgo: 100 ÷ 35 = 2,86. Arriesgaría 100 €: no entra en los 50.
- Se achica: 50 ÷ 35 = **1,43 € por punto**. Pasa el mínimo de US500 (1).
- Si el lugar fuera de 20 €: 20 ÷ 35 = 0,57, por debajo del mínimo de 1: **se rechaza**.

## Ejercicio

Cuatro tamaños, todos con capital 10.000 €, sin posiciones abiertas y sin caída:

1. US500 en 6.000, ATR 20, stop a 20 puntos.
2. US30 en 45.000, ATR 60, stop a 90 puntos.
3. DE40 en 24.000, ATR 30, stop a 45 puntos.
4. US100 en 25.000, ATR 40, stop a 20 puntos.

<details><summary>Ver la solución</summary>

1. 100 ÷ (20 + 5) = **4 € por punto** (tope: 10 × 10.000 ÷ 6.000 = 16,7; no actúa).
2. 100 ÷ (90 + 15) = **0,95 € por punto** (tope: 2,2; no actúa).
3. 100 ÷ (45 + 7,5) = **1,90 € por punto** (tope: 4,2; no actúa).
4. 100 ÷ (20 + 10) = 3,33, pero el tope es 10 × 10.000 ÷ 25.000 = 4: **3,33 € por punto**.

</details>

## Examen

6 preguntas (cálculos y conceptos). Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Ticket de paper**: la calculadora de tamaño (preview) y las órdenes simuladas.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Riesgo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Colchón por gaps | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
| Tamaño = riesgo ÷ (distancia + colchón) | `tamaño = riesgo por operación / (distancia al stop` | `docs/analysis/2026-11-riesgo-propuesta.md` | A |
| Tope del grupo de EE. UU. | 1,5 % (`max_risk_pct = "1.5"`) | `config/risk.toml` | A |
| Instrumentos del grupo | `instruments = ["US100", "US500", "US30", "US2000"]` | `config/risk.toml` | A |
| DE40 y el oro no tienen grupo | `DE40 y XAUUSD` | `config/risk.toml` | A |
| Con lugar parcial se achica; sin lugar, se rechaza | `Si hay lugar parcial, el tamaño se achica` | `config/risk.toml` | A |
| Tope de apalancamiento del US500 | 10:1 (`10`) | `config/instruments.toml` `instruments.US500.max_leverage` | A |
| Tope de apalancamiento del US100 | 10:1 (`10`) | `config/instruments.toml` `instruments.US100.max_leverage` | A |
| Tamaño máximo por apalancamiento | `let max_size = limits.max_leverage * equity / req.entry;` | `engine/crates/risk/src/lib.rs` | A |
| Mínimo del broker en US500 | 1 (`1`) | `config/instruments.toml` `instruments.US500.min_deal_size` | A |
| Mínimo del broker en US100 | 0,5 (`0.5`) | `config/instruments.toml` `instruments.US100.min_deal_size` | A |
| Por debajo del mínimo, se rechaza | `Some(min) if size < min => return Err(Rejection::BelowMinDealSize),` | `engine/crates/risk/src/lib.rs` | A |
| Máximo de posiciones abiertas | 5 (`5`) | `config/risk.toml` `limits.max_open_positions` | A |
| Posiciones por instrumento | 1 (`1`) | `config/risk.toml` `limits.max_positions_per_instrument` | A |
| Orden de los rechazos | `kill switch, drawdown, pérdida diaria, cantidad de posiciones, instrumento abierto, distancia al stop y recién ahí el grupo` | `docs/analysis/2026-11-riesgo-propuesta.md` | A |
