# 7.2 Stop obligatorio y su distancia

> **Nivel 7 · Riesgo y tamaño.** Educativo: no es consejo de inversión ni promete ganancias. Todo
> pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Los precios de los ejemplos
> son redondos, no cotizaciones.

## Objetivo

Poner **siempre** un [stop](../GLOSARIO.md#stop-loss), del lado correcto y a una distancia que el
motor acepte: **entre 0,5 y 3 ATR**. Y saber por qué un [gap](../GLOSARIO.md#gap) puede saltarlo.

## La idea

**Sin stop no hay operación.** Toda operación pasa por el módulo de riesgo, y el riesgo necesita el
stop para calcular el tamaño (lección 7.3). No hay atajo que lo saltee (CLAUDE.md §5).

**Del lado correcto.** En un largo, el stop va **abajo** de la entrada; en un corto, **arriba**. Un
stop del lado equivocado no limita nada.

**Ni muy cerca ni muy lejos, medido en [ATR](../GLOSARIO.md#atr).**

- **Menos de 0,5 ATR:** el stop está dentro del ruido normal de una vela; el motor lo rechaza con
  "stop demasiado cerca".
- **Más de 3 ATR:** el riesgo por punto es tan grande que el tamaño queda diminuto; el motor lo
  rechaza con "stop demasiado lejos".

Con un ATR de 20 puntos, el stop tiene que estar **entre 10 y 60 puntos** de la entrada.

**El stop no es un precio garantizado.** Si el precio salta por encima del stop (un gap del fin de
semana o una noticia), la orden se ejecuta en el primer precio disponible, peor que el stop. Por eso
el motor calcula el tamaño como si el stop estuviera **0,25 ATR más lejos** (el colchón por gaps,
lección 7.3): así un salto de ese tamaño también entra en el presupuesto de riesgo.

**Cuántas veces el riesgo dijo que no.** En la prueba de la estrategia en uso con los topes (R039,
153 meses), el módulo de riesgo rechazó **9.883** señales por "stop demasiado lejos". El rechazo no
es un error: es el filtro haciendo su trabajo.

> **Probá antes de seguir:** ATR de 20. Entrás largo en 6.000 con el stop en 5.945. ¿Lo acepta el
> motor?
>
> <details><summary>Ver la respuesta</summary>
>
> La distancia es 55 puntos = 2,75 ATR, entre 0,5 y 3: **sí**. Con el stop en 5.935 (65 puntos =
> 3,25 ATR), lo rechazaría por demasiado lejos.
>
> </details>

## Interactivo

El ticket en modo preview: escribís entrada y stop, y el motor dice si pasa o por qué no (stop
demasiado cerca, demasiado lejos o del lado equivocado), con la distancia en ATR.

## Ejercicio

ATR de 20 en los tres casos. Ajustá cada ticket hasta que pase:

1. Largo en 6.000 con el stop en 5.995.
2. Largo en 6.000 con el stop en 5.920.
3. Corto en 6.000 con el stop en 5.980.

<details><summary>Ver la solución</summary>

1. 5 puntos = 0,25 ATR: demasiado cerca. El stop tiene que quedar en 5.990 o más abajo (al menos
   10 puntos).
2. 80 puntos = 4 ATR: demasiado lejos. El stop tiene que quedar en 5.940 o más arriba (como mucho
   60 puntos).
3. En un corto el stop va **arriba**: por ejemplo, 6.030 (30 puntos = 1,5 ATR).

</details>

## Examen

**Examen de replay de operaciones.** El motor elige un gráfico real con la fecha oculta (300 velas
de calentamiento y 400 de examen). Vos operás mientras avanzás, y cada entrada se decide al cierre
de una vela y se ejecuta en la apertura de la siguiente. Hacen falta **al menos 10 operaciones**
para que una sola no decida. En **cada** operación el motor revisa el proceso:

- **hay stop**;
- el stop está **del lado correcto** del cierre de la vela de la decisión;
- la distancia está **entre 0,5 y 3 ATR(14)** de esa vela;
- el riesgo es **como mucho el 1 %**;
- escribiste el **plan** antes;
- **una posición a la vez** (ninguna anterior abierta).

Aprobás si se cumplen todas las reglas en todas las operaciones. El resultado en R (con costos)
**se muestra pero no aprueba ni reprueba**: una operación perdedora bien hecha aprueba, una ganadora
sin stop reprueba. La cantidad de operaciones es **provisoria** hasta que el dueño la apruebe.

## Desbloquea

Nada todavía: el ticket de paper se abre en 7.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Stop mínimo | 0,5 ATR (`0.5`) | `config/risk.toml` `stop_distance.min_atr` | A |
| Stop máximo | 3 ATR (`3`) | `config/risk.toml` `stop_distance.max_atr` | A |
| Colchón por gaps | 0,25 ATR (`0.25`) | `config/risk.toml` `gap.gap_atr` | A |
| Rechazos del riesgo: stop demasiado cerca o lejos | `"stop_too_close", "stop_too_far"` | `contracts/ticket_verdict.v1.schema.json` | A |
| Rechazos por stop demasiado lejos con los topes (R039) | 9.883 (`9 883`) | `docs/analysis/2026-11-riesgo-con-sin-topes.md` | A |
| Operaciones mínimas del examen (provisorio) | 10 (`10`) | `config/academy.toml` `replay.trades_per_exam` | A |
| Velas de calentamiento | 300 (`300`) | `config/academy.toml` `replay.warmup_bars` | A |
| Velas del examen | 400 (`400`) | `config/academy.toml` `replay.exam_bars` | A |
| ATR del examen | 14 (`14`) | `config/patterns.toml` `candles.atr_period` | A |
| Riesgo máximo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Reglas de proceso de cada operación | `stop_present, stop_side (contra el cierre de t), stop_in_range` | `engine/crates/academy/src/grade.rs` | A |
| Plan escrito y una posición a la vez | `plan_written, one_position` | `engine/crates/academy/src/grade.rs` | A |
| Al menos N operaciones | `enough_trades` | `engine/crates/academy/src/grade.rs` | A |
| El resultado se muestra y no aprueba | `resultado (se muestra, NO aprueba)` | `engine/crates/academy/src/grade.rs` | A |
| Entrada en la apertura de la vela siguiente | `entrada en la apertura de t+1` | `engine/crates/academy/src/grade.rs` | A |
