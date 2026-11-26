# 10.1 De la idea a la hipótesis

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). Una hipótesis es una
> pregunta: lo normal es que la respuesta sea «no».

## Objetivo

Convertir una idea de «se ve como» en una **regla medible y completa** (entrada, stop, salida,
tamaño, costos y período) y elegir **contra qué compararla**.

## La idea

Una idea («el precio rebota en los soportes», «después de una noticia fuerte sigue la tendencia»)
no se puede probar: cada persona la aplicaría distinto. Una **hipótesis** sí: dos personas con la
misma regla y los mismos datos sacan el mismo resultado.

**1. Las piezas de una regla completa** (son los bloques del Laboratorio):

| Pieza | Qué tiene que decir | Ejemplo |
|---|---|---|
| Mercado | instrumento, timeframe y fuente de velas | US500, 1 h, futuros |
| Entrada | condición medible al **cierre** de una vela y dirección | la EMA de 20 cruza arriba de la de 50 |
| Filtros | cuándo no se entra | solo de 13:30 a 20:00 UTC |
| Stop | distancia medible, entre 0,5 y 3 ATR | 2 × ATR(14) |
| Salida | objetivo, tiempo o los dos | 2 R o a las 8 velas |
| Tamaño | riesgo por operación, nunca más que el del proyecto | 1 % |
| Costos | los del catálogo, nunca en cero | spread, slippage y financiación del CFD |
| Período | dónde se prueba, sin el fuera de muestra | 2013–2025 |

Cada condición se decide al cierre de la vela t y se ejecuta en t+1 (nivel 9). El Laboratorio
limita la complejidad a propósito: **3 condiciones de entrada como mucho**, unidas con «y», y **2
filtros**. Cada condición extra es una puerta más para el sobreajuste.

**2. De «se ve como» a una regla.** «Comprar cuando rebota fuerte en un soporte» necesita:

- qué es un soporte (en el motor: un nivel tocado al menos k veces con tolerancia X, nivel 4);
- qué es rebotar «fuerte» (por ejemplo, una vela que cierra arriba del nivel con un cuerpo mayor a
  0,5 ATR después de haber tocado el nivel);
- dónde va el stop, cuándo se sale, con qué tamaño y en qué período.

Recién ahí se puede probar, y recién ahí se puede **fallar**, que es lo que la vuelve útil.

**3. Contra qué compararla.** Una regla «gana» o «pierde» frente a algo:

- **Comprar y mantener** con los mismos costos: los tres índices dieron +1,10 % por mes en
  2013–2025 (R018). Una regla de compras que gana menos solo cobra una parte de la suba.
- **Un placebo**: precios cualquiera parecidos (programa 5). Si la señal no hace más que el
  placebo, no aporta nada.
- **Entradas al azar** con las mismas salidas y el mismo tamaño (el test de suerte, el modelo nulo
  de Tom Basso): si la regla no les gana, la entrada no aporta.

> **Probá antes de seguir:** «Vender cuando el mercado está sobrecomprado». ¿Qué falta?
>
> <details><summary>Ver la respuesta</summary>
>
> Todo: qué mide «sobrecomprado» (con qué indicador, qué período y qué umbral, calculado por el
> motor), en qué vela se decide, stop, salida, tamaño, costos, período y contra qué se compara.
>
> </details>

## Interactivo

La plantilla de hipótesis: las ocho piezas de la tabla, con el catálogo de bloques que el motor
acepta hoy en el Laboratorio (cruce de medias y ruptura del rango de apertura para entrar, filtro de
horas, stop en ATR, salida por objetivo o tiempo).

## Ejercicio

Convertí estas tres frases en reglas completas:

1. «Cuando la tendencia es alcista, comprar los retrocesos.»
2. «El oro sube cuando hay miedo.»
3. «La apertura de Nueva York marca la dirección del día.»

<details><summary>Ver la solución</summary>

Hay muchas respuestas válidas; lo que importa es que estén todas las piezas. Por ejemplo:

1. Largo en US500 de 1 h si la EMA de 50 diaria sube y el cierre de 1 h toca la EMA de 20 de 1 h
   desde arriba; entrada en la apertura siguiente; stop a 1,5 ATR(14); salida en 2 R o a las 16
   velas; 1 %; 2013–2025.
2. Largo en oro al cierre del día en que el VIX cierra arriba de 30 (régimen de estrés, nivel 6);
   salida a los 10 días o con stop a 2 ATR diario; 1 %; comparar con comprar y mantener oro.
3. Si la primera vela de 15 m de la sesión de contado cierra arriba de su apertura, largo en la
   apertura de la siguiente; salida al cierre de la sesión; stop a 1 ATR(14); 1 %; comparar con
   entradas al azar a la misma hora.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

Nada nuevo: el Laboratorio se abre en la lección 10.3.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Bloques de una estrategia del Laboratorio | `Mercado`, entrada, filtros, stop, salida, tamaño y costos | `docs/design/2026-11-laboratorio.md` | A |
| Condiciones de entrada como máximo | 3 (`3`) | `config/lab.toml` `limits.max_entry_conditions` | A |
| Filtros como máximo | 2 (`2`) | `config/lab.toml` `limits.max_filters` | A |
| Distancia del stop: mínimo | 0,5 ATR (`0.5`) | `config/risk.toml` `stop_distance.min_atr` | A |
| Distancia del stop: máximo | 3 ATR (`3`) | `config/risk.toml` `stop_distance.max_atr` | A |
| Riesgo por operación | 1 % (`1`) | `config/risk.toml` `limits.risk_per_trade_pct` | A |
| Filtro de horas propuesto | `from = { default = "13:30" }` | `config/lab.toml` | A |
| Comprar y mantener los tres índices | `+1.0955 %` | `docs/analysis/registro-de-resultados.csv` `R018` | A |
| Placebo emparejado | `5 placebos emparejados` | `docs/analysis/registro-de-resultados.csv` `R035` | A |
| Modelo nulo de entradas al azar | `Modelo nulo de Tom Basso` | `docs/analysis/2026-11-autoanalisis.md` | C |
| Bloques que el motor acepta hoy | `ma_cross` | `config/lab.toml` | A |
