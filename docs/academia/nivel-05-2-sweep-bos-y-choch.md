# 5.2 Sweep, BOS y CHoCH

> **Nivel 5 · Liquidez.** Educativo: no es consejo de inversión ni promete ganancias. Todo pasa en
> simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos son
> redondos, no cotizaciones.

## Objetivo

Reconocer con la definición medible del motor una **barrida** ([sweep](../GLOSARIO.md#sweep)), una
**ruptura de estructura** ([BOS](../GLOSARIO.md#bos)) y un **cambio de carácter**
([CHoCH](../GLOSARIO.md#choch)), y saber en qué vela existen.

## La idea

Estas palabras vienen del mundo ICT/SMC ([liquidez ICT](../GLOSARIO.md#liquidez-ict)). Ahí se usan
muchas veces "a ojo". El motor las fija así (spec §5.4); acá va la versión para largos y la de
cortos es el espejo:

**Sweep (barrida de liquidez externa).**

```
P = último pivote bajo de 1 h confirmado al cierre de la vela t (de 15 m)
sweep: L[t] < P y C[t] > P      (pinchó por debajo y volvió a cerrar adentro)
```

El pivote de 1 h tiene que estar **confirmado** (su vela de confirmación cerró): antes no se sabe
que es pivote. En el puntaje de la estrategia en uso, un sweep a favor suma 2.

**BOS y CHoCH (rupturas de estructura).**

```
P = último pivote alto de 15 m confirmado
ruptura alcista en t: C[t] > P y C[t−1] ≤ P
  BOS   si la tendencia de 1 h es ALCISTA  (continuación)
  CHoCH si la tendencia de 1 h es BAJISTA  (posible giro)
  con tendencia NEUTRAL no es ninguna de las dos
```

- Cada pivote se rompe **una sola vez**: si el precio vuelve y lo cruza de nuevo, no es otra
  ruptura.
- La tendencia de 1 h es la de la lección 4.3, de la **última vela de 1 h cerrada**.
- Un CHoCH cuenta como "reciente" durante **12 velas** (la actual incluida) y suma 1 al puntaje.

**El volumen de la barrida** (solo contexto): volumen de la vela del sweep ÷ mediana del volumen
de las **20 velas** anteriores. Si la fuente no trae volumen (las velas de IG pueden no tenerlo),
queda vacío: no se inventa.

**Lo que las definiciones no dicen.** Que haya un sweep o un CHoCH no quiere decir que el precio
vaya a girar. Son definiciones para poder contar y medir; sobre ICT/SMC **no encontramos estudios
revisados**, y lo que medimos nosotros con el FVG (lección 5.4) no dio a favor.

> **Probá antes de seguir:** el último pivote bajo de 1 h confirmado está en 5.980. Una vela de 15 m
> tiene mínimo 5.976 y cierra en 5.978. ¿Es un sweep?
>
> <details><summary>Ver la respuesta</summary>
>
> No: pinchó por debajo (5.976 < 5.980) pero **cerró abajo** (5.978 < 5.980). Para el mapa de
> liquidez, ese mínimo quedó **roto**, no barrido.
>
> </details>

## Interactivo

Un replay con los eventos marcados en el momento en que se confirman: el sweep al cierre de su vela
y el BOS o CHoCH al cierre de la vela que rompe, nunca antes.

## Ejemplo resuelto

Último pivote alto de 15 m confirmado: 6.040. Tendencia de 1 h: BAJISTA.

1. Vela A cierra en 6.038 (la anterior en 6.030): no rompe.
2. Vela B cierra en 6.044, con la anterior (A) en 6.038 ≤ 6.040: **ruptura alcista**. Con
   tendencia de 1 h bajista, es un **CHoCH**.
3. Vela C cierra en 6.036 y vela D en 6.046: el precio volvió a cruzar el mismo pivote, pero **no**
   es otra ruptura (cada pivote se rompe una sola vez). Haría falta un pivote alto nuevo,
   confirmado, para que haya otra.
4. El CHoCH de B cuenta como reciente hasta la vela número 12 desde B (B incluida).

## Ejercicio

En un replay de práctica, marcá con una nota cada sweep que veas y escribí el pivote de 1 h que
barrió. Después prendé la capa del motor y compará.

<details><summary>Ver la solución</summary>

Los errores más comunes: usar un pivote de 1 h todavía sin confirmar, llamar "sweep" a una vela que
cerró del otro lado (eso es una ruptura), o usar un pivote de 15 m en lugar del de 1 h.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio). (El temario pedía un replay donde
marcar 5 eventos; el motor todavía no califica marcas de sweeps ni de rupturas, así que por ahora
son preguntas.)

## Desbloquea

- **Capas de sweep y estructura**: barridas, BOS y CHoCH del motor sobre el gráfico.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Sweep: pinchó y volvió a cerrar adentro | `L[t] < P y C[t] > P` | `docs/spec/fase-1-especificacion.md` §5.4 | A |
| Sweep: el pivote de 1 h tiene que estar confirmado | `último pivote bajo confirmado de 1h` | `docs/spec/fase-1-especificacion.md` §5.4 | A |
| Ruptura: cierre arriba con el anterior en el pivote o abajo | `C[i] > P y C[i−1] ≤ P` | `engine/crates/liquidity/src/structure.rs` | A |
| Cada pivote se rompe una sola vez | `Cada pivote se rompe una sola vez` | `engine/crates/liquidity/src/structure.rs` | A |
| Con tendencia neutral no es BOS ni CHoCH | `con tendencia NEUTRAL no es ninguna de las dos` | `engine/crates/liquidity/src/structure.rs` | A |
| CHoCH reciente: últimas 12 velas | 12 (`12`) | `config/liquidity.toml` `structure.choch_recent_bars` | A |
| Puntaje de un sweep | +2 (`2`) | `config/strategy.toml` `score.sweep` | A |
| Puntaje de un CHoCH reciente | +1 (`1`) | `config/strategy.toml` `score.recent_choch` | A |
| Volumen de la barrida: mediana de las velas anteriores | 20 (`20`) | `config/liquidity.toml` `sweep_volume.median_bars` | A |
| Sobre ICT/SMC no hay estudios revisados: se enseña como definición | `Sobre ICT/SMC no hay estudios revisados` | `docs/design/2026-11-academia.md` §7 | C |
