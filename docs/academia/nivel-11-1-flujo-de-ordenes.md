# 11.1 Flujo de órdenes y desequilibrio aproximado

> **Nivel 11 · Avanzado, con advertencias.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los números
> de los ejemplos son redondos, de práctica: no son cotizaciones ni resultados.

## Objetivo

Saber qué es el **libro de órdenes** y el **delta**, qué datos hacen falta para medirlos, y por qué
nuestro índice de desequilibrio es una **aproximación desde el precio**, no flujo de órdenes.

## La idea

**1. El libro y quién agrede.** En una bolsa (por ejemplo, los futuros del CME) hay órdenes
**límite** esperando a un precio: las de compra y las de venta forman el **libro de órdenes**. Una
orden **de mercado** se ejecuta contra ese libro: el que la manda es el **agresor**. El *flujo de
órdenes* es esa secuencia de ejecuciones, con su lado (Harris, 2003; O'Hara, 1995).

**2. El delta.** En una vela:

delta = contratos comprados por agresores − contratos vendidos por agresores

Para calcularlo hace falta **cada operación con su lado**. Muchas veces el lado no viene en el dato
y se infiere comparando el precio con la mejor compra y la mejor venta del momento. Lee y Ready
(1991) estudiaron esa clasificación y sus problemas: la cotización puede quedar registrada antes de
la operación que la provocó, y una operación hecha **dentro** del spread no tiene un lado claro. O
sea: aun con datos completos, el delta es una estimación.

**3. Lo que tenemos y lo que no.**

| Dato | ¿Lo tenemos? |
|---|---|
| Libro de órdenes (precios y cantidades esperando) | **No** |
| Cada operación con su lado (para el delta) | **No** |
| [Volumen](../GLOSARIO.md#volumen) por vela del CFD de IG | **No**: IG no da volumen ni libro |
| Velas de 1 minuto de los futuros, con su volumen | **Sí** (Databento, esquema `ohlcv-1m`): cuánto se operó, no quién agredió |

Así que en esta app **no hay flujo de órdenes real**: ni delta, ni *footprint*, ni libro. Sumar
otro esquema de datos sería un proveedor o un dato nuevo, y eso va con su ADR (CLAUDE.md §6).

**4. Lo que sí hay: el índice de desequilibrio aproximado.** El motor lo calcula desde las velas
y lo manda **siempre** con su rótulo, «aproximación desde el precio»:

- **Vela de desplazamiento:** cuerpo |C − O| ≥ **1,5 ×** [ATR](../GLOSARIO.md#atr)(14) **y**
  cuerpo ≥ **70 %** del rango (H − L). Es alcista si cierra arriba de la apertura.
- **Índice** en la vela t, con las últimas **20** velas cerradas (la última es t):

  índice = (Σ cuerpos de desplazamiento alcistas − Σ bajistas) ÷ Σ (H − L) de las 20 velas

  Va de −1 a +1; es 0 si el rango sumado es 0. No sale hasta que la vela más vieja de la ventana
  tiene ATR (calentamiento) y el valor de una vela cerrada no cambia nunca (sin repintado).

*Ejemplo:* en 20 velas el rango sumado es 400 puntos; hubo dos velas de desplazamiento alcistas
con cuerpos de 60 y 40 y una bajista de 20. Índice = (100 − 20) ÷ 400 = **+0,2**.

**5. Las advertencias.**

- Una vela grande verde no dice **quién** compró: puede ser una compra fuerte, stops de vendedores
  que saltan o cortos que cierran. El índice mide el **cuerpo de las velas**, no órdenes.
- Es **solo informativo**: ninguna señal ni estrategia del motor lo usa, y **no lo probamos como
  señal**: no hay ninguna fila del registro de resultados que diga que da ventaja.
- Los videos que muestran *footprint* o delta usan datos que acá no existen. Lo de Emre (un trader
  de opciones que el dueño trajo) es un ejemplo: discrecional, con opciones y datos pagos.

> **Probá antes de seguir:** el índice marca +0,6 en US100. ¿Podés decir que «entraron
> compradores institucionales»?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Dice que en las últimas 20 velas los cuerpos de desplazamiento alcistas fueron grandes
> comparados con el rango. Quién compró, cuánto y con qué órdenes no se puede saber sin el libro ni
> las operaciones con su lado.
>
> </details>

## Interactivo

En la vista **Liquidez**, la tarjeta «Índice de desequilibrio» (con su rótulo) y las marcas de
desplazamiento sobre el gráfico, con los valores que manda el motor.

## Ejercicio

Calculá el índice de una ventana de 20 velas con rango sumado de 500 puntos, dos cuerpos de
desplazamiento bajistas de 45 y 55, y ninguno alcista.

<details><summary>Ver la solución</summary>

(0 − 100) ÷ 500 = **−0,2**. Las otras 17 velas cuentan en el rango, aunque no sean de
desplazamiento.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Índice de desequilibrio aproximado** (vista Liquidez), siempre con su rótulo.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Libro de órdenes, agresor y flujo de órdenes | — | [O'Hara (1995), *Market Microstructure Theory*, Blackwell (hoy Wiley)](https://www.wiley-vch.de/en/areas-interest/finance-economics-law/market-microstructure-theory-978-1-55786-443-7), ficha leída el 26-11-2026 | B |
| Órdenes límite y de mercado | — | [Harris, *Trading and Exchanges*, Oxford University Press (2002/2003)](https://ideas.repec.org/b/oxp/obooks/9780195144703.html) | B |
| Clasificar el lado de cada operación y sus dos problemas | — | [Lee y Ready (1991), *Inferring Trade Direction from Intraday Data*, Journal of Finance 46(2), 733–746](https://ideas.repec.org/a/bla/jfinan/v46y1991i2p733-46.html), resumen leído el 26-11-2026 | A |
| IG no da volumen ni libro | `no da volumen ni libro de órdenes` | `engine/crates/liquidity/src/imbalance.rs` | A |
| Futuros: solo velas de 1 minuto | `ohlcv-1m` | `config/databento.toml` `download.schema` | A |
| Rótulo obligatorio | `aproximación desde el precio` | `engine/crates/core/src/imbalance.rs` | A |
| Cuerpo mínimo de desplazamiento, en ATR | 1,5 (`1.5`) | `config/liquidity.toml` `imbalance.displacement_body_atr` | A |
| ATR de 14 velas | `displacement_body_atr × ATR14` | `config/liquidity.toml` | A |
| Cuerpo mínimo sobre el rango | 70 % (`0.7`) | `config/liquidity.toml` `imbalance.min_body_ratio` | A |
| Velas de la ventana | `20` | `config/liquidity.toml` `imbalance.window_bars` | A |
| Fórmula del índice | `(Σ cuerpos de desplazamiento alcistas − Σ bajistas) / Σ (H − L) de la ventana` | `engine/crates/liquidity/src/imbalance.rs` | A |
| Solo informativo | `Solo informativo` | `engine/crates/liquidity/src/imbalance.rs` | A |
| Lo de Emre no se puede copiar | `es discrecional, usa opciones y datos pagos` | `docs/research/videos/2026-11-25-emre-vs-gianluca.md` | C |
