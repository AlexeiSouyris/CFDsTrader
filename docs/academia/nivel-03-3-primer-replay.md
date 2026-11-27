# 3.3 Primer replay

> **Nivel 3 · Velas y timeframes.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Son velas reales del
> pasado: lo que pasó en ellas no dice nada seguro del futuro.

## Objetivo

Avanzar por un gráfico real **vela por vela, sin ver el futuro**, y escribir qué esperás **antes**
de avanzar.

## La idea

Mirar un gráfico viejo entero engaña: el final está a la vista y todo "parece obvio". El replay
saca esa ventaja: te muestra solo el pasado hasta un punto (el **cursor**) y vos decidís sin saber
qué sigue.

**Cómo funciona:**

1. El motor elige al azar un instrumento (US500, US100, US30 o DE40, con velas de los futuros de
   Databento), un timeframe (15 m o 1 h) y una fecha.
2. **La fecha queda oculta**: ves el número de día y la hora, no el año. El precio es el real (así
   los números redondos siguen siendo redondos). La fecha real se revela al final.
3. Primero ves **300 velas de calentamiento** para tener contexto.
4. Avanzás de a una vela o de a varias (hasta 100 por paso). La práctica libre tiene **200 velas**.

**El cursor vive en el motor.** La UI no tiene las velas de después: las pide y el motor manda solo
hasta el cursor. No hay forma de "espiar". Si marcás una orden en la vela `t`, se ejecuta en la
apertura de `t+1`, igual que en el backtest y en el paper.

**El pasado que no se toca.** El sorteo de fechas sale solo del período de investigación: nunca de
los **últimos 12 meses** de datos, que quedan fuera de muestra para juzgar estrategias al final.

**Por qué escribir antes.** Si no escribís qué esperás antes de avanzar, después es fácil creer que
"lo sabías". Escribirlo te deja comparar lo que pensaste con lo que pasó, sin trampa de la memoria.

> **Probá antes de seguir:** en el replay marcás "comprar" en la vela 340. ¿A qué precio entrás?
>
> <details><summary>Ver la respuesta</summary>
>
> A la apertura de la vela 341 (más el costo). La vela 340 ya cerró cuando decidiste: su cierre no
> está disponible para ejecutar.
>
> </details>

## Interactivo

Un replay de práctica, sin nota: 300 velas de calentamiento y 200 para avanzar, con la fecha oculta.
Podés marcar niveles y medir distancias con la regla; nada se califica.

## Ejercicio

Antes de avanzar 20 velas, escribí en una línea qué esperás (por ejemplo: "sigue bajando hasta el
mínimo de ayer" o "no sé, está en rango"). Avanzá las 20 velas y anotá si pasó, si no pasó, o si tu
frase no se podía comprobar.

<details><summary>Ver la solución</summary>

No hay respuesta correcta: el ejercicio es el hábito. Una buena frase se puede comprobar (dice un
precio o una condición). "Va a subir fuerte" no se puede comprobar; "toca 6.050 antes que 5.980"
sí.

</details>

## Examen

Esta lección no tiene examen: es práctica libre, sin nota. Se completa al leerla; el replay de
práctica se puede repetir todas las veces que quieras.

## Desbloquea

- **Replay libre**: practicar cuando quieras, sin nota.
- **Regla de medición**: medir distancias en puntos y en tiempo sobre el gráfico.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Velas de calentamiento del replay | 300 (`300`) | `config/academy.toml` `replay.warmup_bars` | A |
| Velas de una práctica libre | 200 (`200`) | `config/academy.toml` `replay.practice_bars` | A |
| Velas por paso como máximo | 100 (`100`) | `config/academy.toml` `replay.max_step_bars` | A |
| Velas de los futuros | `futures` | `config/academy.toml` `replay.source` | A |
| Proveedor de las velas | `databento` | `config/academy.toml` `replay.futures_source` | A |
| Instrumentos del replay | `instruments = ["US500", "US100", "US30", "DE40"]` | `config/academy.toml` | A |
| Timeframes del replay | `timeframes = ["15m", "1h"]` | `config/academy.toml` | A |
| Meses que nunca se sortean (fuera de muestra) | 12 (`12`) | `config/data_split.toml` `out_of_sample.months` | A |
| Orden marcada en `t`: entra en la apertura de `t+1` | `se ejecuta en la apertura de` | `docs/design/2026-11-academia.md` §5.3 | A |
| La fecha real se revela al final | `al final se revela la fecha real` | `docs/design/2026-11-academia.md` §5.1 | A |
