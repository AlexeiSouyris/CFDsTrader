# 9.1 Mirar el futuro y repintar

> **Nivel 9 · Backtesting y sus trampas.** Educativo: no es consejo de inversión ni promete
> ganancias. Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Un
> [backtest](../GLOSARIO.md#backtest) es el pasado simulado: no dice nada seguro del futuro.

## Objetivo

Encontrar el error en una regla que usa información que **todavía no existía** cuando había que
decidir: decidir al cierre de t, ejecutar en t+1, solo velas cerradas y sin
[repintado](../GLOSARIO.md#repintado).

## La idea

Un backtest con [look-ahead](../GLOSARIO.md#look-ahead) («mirar el futuro») siempre se ve bien,
porque la regla sabe algo que en vivo no sabrías. Es el error más caro: no se nota en la curva, se
nota cuando la plata es real. Por eso son reglas del proyecto (CLAUDE.md §5b) y el motor las cumple
por construcción.

**1. Decidir al cierre de t, ejecutar en t+1.** El cierre de una vela se conoce cuando la vela
**terminó**. Si la regla mira ese cierre, la orden va en la **apertura de la vela siguiente**. El
motor de investigación lo hace así: «entrada en la apertura de t+1». Comprar «al mismo cierre» que
dio la señal es mirar el futuro, aunque sea por un instante.

**2. Solo velas cerradas.** Ningún indicador ni patrón usa la vela en curso. En varios
timeframes, la vela mayor se usa recién cuando cerró: a las 10:40 UTC, la última vela de 1 h que
existe es la de 09:00 a 10:00. La de 10:00 a 11:00 todavía puede cambiar.

**3. Pivotes confirmados.** Un máximo local con N velas de cada lado (N = 3 en el motor) existe
recién cuando cerraron las N velas siguientes. La señal va en la vela de confirmación, no en la del
máximo. Un backtest que entra «en el pivote» está usando 3 velas del futuro.

**4. Sin repintado.** Un valor calculado para una vela cerrada **no cambia nunca** al llegar datos
nuevos. Si un indicador «se corrige» para atrás, el gráfico muestra señales que en su momento no
estaban. El motor tiene tests que agregan velas y verifican que nada de lo anterior cambie (34 tests
de no repintado, según el autoanálisis), y el Laboratorio tiene un test de propiedad: cortar la
serie en cualquier vela no cambia los planes anteriores.

**5. Otras formas de mirar el futuro, menos obvias:**

- **El mínimo o el máximo del día** antes de que el día termine.
- **Noticias**: una noticia influye desde su **hora de publicación real**, no desde la hora del
  evento que describe.
- **Calentamiento**: un indicador no emite señal hasta tener datos suficientes; usarlo antes es
  inventar los datos que faltan.
- **Elegir qué probar con el diario de hoy**: las acciones que hoy están en un índice (lección 9.2).
- **Ajustar la regla mirando el período de prueba**: es mirar el futuro con más pasos (lección 9.4).

> **Probá antes de seguir:** regla: «si la vela diaria de hoy cierra arriba del máximo de ayer,
> comprar a las 15:00 de hoy». ¿Dónde está el futuro?
>
> <details><summary>Ver la respuesta</summary>
>
> A las 15:00 el cierre diario de hoy todavía no existe. Versión válida: «si el cierre de **ayer**
> quedó arriba del máximo de anteayer, comprar en la apertura de hoy», o mirar a las 15:00 el
> precio de una vela que **ya cerró** (por ejemplo, la de 14:45 a 15:00).
>
> </details>

## Interactivo

Tres reglas escritas con un error escondido y, al lado, la versión correcta. El motor solo sabe
correr la correcta: no tiene un modo con look-ahead, a propósito. En el replay (lección 3.3) se ve
lo mismo desde el otro lado: el motor no manda una vela que todavía no debería verse.

## Ejercicio

Encontrá el error en cada regla:

1. «Vender cuando el cierre de 1 h cruza abajo de la media de 20; la orden va en la vela de 1 h que
   está corriendo.»
2. «Stop en el último pivote de 15 m, con el pivote que marca el gráfico ahora.»
3. «Entrar el día del CPI a las 08:00 ET si el dato salió mejor de lo esperado.»

<details><summary>Ver la solución</summary>

1. La vela de 1 h que está corriendo no cerró: la señal se decide al cierre de esa vela y la orden
   va en la siguiente.
2. Solo valen los pivotes **confirmados** hasta ese momento (3 velas después); el último que dibuja
   el gráfico puede no estar confirmado todavía.
3. El dato se publica a las 08:30 ET: a las 08:00 no se sabe si salió mejor. Además, «lo esperado»
   tiene que estar guardado con su hora real; si no lo tenemos, la regla no se puede probar.

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Corridas de backtest** (Performance): las corridas guardadas del motor, con su config, sus
  métricas y su detalle.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Regla: nada de mirar el futuro | `Nada de mirar el futuro (look-ahead):` | `CLAUDE.md` | A |
| Regla: solo velas cerradas | `Solo velas cerradas.` | `CLAUDE.md` | A |
| Regla: sin repintado | `un valor calculado para una vela cerrada no cambia nunca al llegar datos nuevos` | `CLAUDE.md` | A |
| Regla: pivotes confirmados | `un máximo o mínimo local solo existe después de N velas de confirmación` | `CLAUDE.md` | A |
| Regla: noticias con hora de publicación | `una noticia solo influye desde el momento en que estuvo disponible` | `CLAUDE.md` | A |
| Regla: calentamiento | `un indicador no emite señal hasta tener datos suficientes` | `CLAUDE.md` | A |
| Ejecución del motor de investigación | `entrada en la apertura de t+1` | `engine/crates/backtest/src/research/run.rs` | A |
| Velas de cada lado de un pivote | 3 (`3`) | `config/patterns.toml` `pivots.strength` | A |
| Tests de no repintado | `34 tests` | `docs/analysis/2026-11-autoanalisis.md` | A |
| Hora de los datos de CPI y empleo | 08:30 ET (`08:30`) | `config/research.toml` `macro_day.release_time` | A |
| Test de propiedad del Laboratorio | `cortar la serie en cualquier vela no cambia los planes anteriores` | `docs/modules/lab.md` | A |
