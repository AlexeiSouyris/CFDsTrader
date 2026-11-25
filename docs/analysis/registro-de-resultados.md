# Registro de resultados

Issue #514 · creado el 24-11-2026, a pedido del dueño: "quiero que guardes también los resultados que tenemos ahora, y que si llegamos a cambiar algo, la diferencia de resultados, si dio mejor o si dio peor".

Los datos están en [`registro-de-resultados.csv`](registro-de-resultados.csv), con **una fila por prueba**. Cada fila apunta al informe completo y al Issue o PR. Nada de esto es una recomendación de inversión, y los resultados pasados no garantizan resultados futuros.

## Cómo se lee

| Columna | Qué dice |
|---|---|
| `id` | Número fijo de la fila (R001, R002…). Nunca se reutiliza. |
| `programa` | De dónde sale: estrategia en uso, ML, programa de investigación y ronda, o referencia. |
| `metrica` y `valor_escenario_eur` / `valor_escenario_usd` | La cifra principal. Hay dos valores porque IG todavía no confirmó qué tasa usa para financiar los índices de EE. UU., y se prueba con las dos. |
| `veredicto` | Pasa / no pasa / pierde / no mejora / referencia, según la regla fijada **antes** de correr. |
| `compara_con` | Si la fila repite o cambia una prueba anterior: el `id` de esa fila. |
| `cambio` | **mejor**, **peor** o **igual** frente a esa fila. |
| `diferencia` | Cuánto cambió, en palabras simples. |

## La regla (para personas y agentes)

1. **Las filas viejas no se editan ni se borran.** Si un resultado se corrige (un error en el código, datos nuevos, otro costo), se agrega una fila **nueva** con `compara_con` apuntando a la vieja, y se dice si dio mejor, peor o igual.
2. **Toda prueba nueva agrega su fila** en el mismo PR que su informe, aunque no pase.
3. Un cambio de parámetro en la estrategia en uso también agrega una fila, que compara contra la última fila de la estrategia en uso del mismo instrumento.
4. "Mejor" quiere decir mejor en la métrica de la fila, no que la idea pase. Una idea puede mejorar y seguir sin pasar.
5. Los chequeos **descriptivos** (que no votan) también van, marcados como "descriptivo" en `programa`.

## Resumen al 24-11-2026

- **Estrategia en uso (15 minutos):** pierde en US500, US100, US30, US2000 y DE40. Antes de costos queda cerca de cero; los costos hacen la pérdida (R001–R009).
- **ML:** ningún modelo supera a la tabla base B1 (R010–R013).
- **Investigación:** 6 ideas probadas con reglas fijadas antes, y ninguna pasa (R014–R017, R019–R020).
- **Programa Nasdaq (#526):** H1, deriva antes del comunicado del FOMC en US100, no pasa (R027; R028 descriptivo en US500 y US30). H2 espera el calendario del BLS.
- **Programa 4 (#561):** estrategia A de Gianluca Bruni en US100, literal (H4.1, 15 puntos) y normalizada (H4.2, 0,35 × ATR): ninguna pasa (R029, R030). H4.3 espera el calendario del BLS.
- **Referencia:** comprar y mantener los tres índices con los costos del CFD dio +1,10 % por mes (R018). Ninguna idea lo superó.
- **Ejemplo de comparación:** con 8 ventanas, US100 parecía estar en cero (R006). Con la historia completa, pierde −6,1 % por ventana (R007, **peor**): las 8 ventanas eran un tramo favorable.
