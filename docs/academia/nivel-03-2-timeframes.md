# 3.2 Timeframes

> **Nivel 3 · Velas y timeframes.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)).

## Objetivo

Ver el mismo momento en velas de 1 minuto, 15 minutos, 1 hora y 1 día, y saber que la vela de un
[timeframe](../GLOSARIO.md#timeframe) mayor solo vale cuando **cerró**.

## La idea

El **timeframe** es cuánto tiempo resume cada vela. El precio es el mismo; cambia el zoom:

| Timeframe | Una vela resume | Para qué sirve |
|---|---|---|
| 1 m | 1 minuto | Ver cómo se movió el precio por dentro. |
| 15 m | 15 minutos | El timeframe de las señales de la estrategia en uso. |
| 1 h | 1 hora (4 velas de 15 m) | El contexto: la tendencia de 1 h. |
| 1 d | Una sesión | El cuadro grande. |

**Las velas mayores se arman con las menores.** Una vela de 1 h es la unión de 4 de 15 m: abre con
la apertura de la primera, cierra con el cierre de la última, y su máximo y su mínimo son los
extremos de las cuatro. Los futuros de 1 minuto se agrupan así a 15 m, 1 h y 1 d en el motor
(Rust); la UI no arma velas.

**La vela mayor existe recién cuando cerró.** La vela de 1 h de las 15:00 (de 15:00 a 16:00) recién
existe a las 16:00. A las 15:40 hay tres velas de 15 m cerradas de esa hora, pero la de 1 h todavía
está en formación: su cierre puede cambiar. Si una señal de 15 m mirara esa vela de 1 h a medio
hacer, estaría usando un dato que todavía no existía: es mirar el futuro
([look-ahead](../GLOSARIO.md#look-ahead)).

**Un ejemplo del motor.** La tendencia de 1 h compara el cierre de la **última vela de 1 h
cerrada** con su media de 50 velas ([SMA](../GLOSARIO.md#sma)) y mira si esa media subió frente a la
de 5 velas antes. Si la última vela de 1 h cerrada es la de las 09:00, a las 10:40 se usa esa, no la
de las 10:00.

**Calentamiento.** Una media de 50 velas necesita 50 velas. El motor no emite ninguna señal hasta
tener **200 velas de 15 m** y **60 de 1 h** cerradas y válidas. Antes de eso, no hay señal: no es
"señal débil", es ninguna.

> **Probá antes de seguir:** son las 10:40 UTC. ¿Cuál es la última vela de 15 m cerrada?
>
> <details><summary>Ver la respuesta</summary>
>
> La de las 10:15 (de 10:15 a 10:30). La de las 10:30 cierra a las 10:45.
>
> </details>

## Interactivo

Multigráfico de 2 × 2 con el mismo instrumento en 1 m, 15 m, 1 h y 1 d, y el cursor sincronizado:
al mover el cursor en uno, los otros marcan el mismo momento. Se ve cómo una vela de 1 h "contiene"
a cuatro de 15 m.

## Ejercicio

Son las 10:40 UTC de un martes. Decí qué vela es la última **cerrada** en 15 m, en 1 h y en 1 d.

<details><summary>Ver la solución</summary>

- 15 m: la de las 10:15 (cerró a las 10:30).
- 1 h: la de las 09:00 (cerró a las 10:00). La de las 10:00 está en formación hasta las 11:00.
- 1 d: la de la sesión anterior (la de hoy no terminó).

</details>

## Examen

6 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Selector de timeframe** en los gráficos.
- **Multigráfico**: varios gráficos a la vez con el cursor sincronizado.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| La vela de 1 h se arma con las de 15 m y existe al cerrar | `La vela de 1h de las 15:00 recién existe a las 16:00` | `docs/spec/fase-1-especificacion.md` §3 | A |
| Calentamiento en 15 m | 200 velas (`200`) | `config/strategy.toml` `evidence.min_m15_candles` | A |
| Calentamiento en 1 h | 60 velas (`60`) | `config/strategy.toml` `evidence.min_h1_candles` | A |
| Media de la tendencia de 1 h | 50 velas (`50`) | `config/indicators.toml` `trend.sma_period` | A |
| Pendiente de esa media: contra 5 velas antes | 5 (`5`) | `config/indicators.toml` `trend.slope_lookback` | A |
| Futuros de 1 m agrupados en el motor | `agrupados a 15 m, 1 h y 1 d` | `docs/design/2026-11-academia.md` §5.2 | A |
| Timeframes de los exámenes de replay | `timeframes = ["15m", "1h"]` | `config/academy.toml` | A |
