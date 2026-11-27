# Registro de resultados

Issue #514 · creado el 24-11-2026, a pedido del dueño: "quiero que guardes también los resultados que tenemos ahora, y que si llegamos a cambiar algo, la diferencia de resultados, si dio mejor o si dio peor".

Los datos están en [`registro-de-resultados.csv`](registro-de-resultados.csv), con **una fila por prueba**. Cada fila apunta al informe completo y al Issue o PR. Nada de esto es una recomendación de inversión, y los resultados pasados no garantizan resultados futuros.

## Cómo se lee

| Columna | Qué dice |
|---|---|
| `id` | Número fijo de la fila (R001, R002…). Nunca se reutiliza. |
| `programa` | De dónde sale: estrategia en uso, ML, programa de investigación y ronda, o referencia. |
| `metrica` y `valor_escenario_eur` / `valor_escenario_usd` | La cifra principal. Hay dos valores porque, cuando se corrieron, IG todavía no había confirmado qué tasa usa para financiar los índices de EE. UU. y se probó con las dos. IG confirmó SOFR el 24-11-2026 (#655): para US500, US100 y US30 vale la columna USD (EFFR hasta 2018-03, SOFR desde 2018-04); la EUR queda como registro. |
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
6. **Toda hipótesis que vota paga su nivel del presupuesto global** (sección siguiente) en el mismo PR que la congela. No hay "5 % nuevo" por programa.

## Presupuesto global

Regla aprobada por el dueño el 25-11-2026 en #660 (cita: «1, 2 y 7 apruebo la recomendacion.», [comentario](https://github.com/AlexeiSouyris/CFDsTrader/issues/660#issuecomment-7169990064)); detalle en [`2026-11-regla-presupuesto-global.md`](2026-11-regla-presupuesto-global.md) y adopción en #690. Los números están en `config/research.toml`, `[alpha_budget]`.

- **Un solo presupuesto para todo el proyecto**, con alfa-inversión (Foster y Stine, 2008) y α = 5 %: investigación **y** pruebas confirmatorias de ML salen de la misma riqueza `W`. Arranca en **W(0) = 4,75 %** el 25-11-2026. Lo gastado antes (18,75 % en investigación, 10 % en ML) no se arrastra como deuda, pero sigue contando en el N del Sharpe deflactado.
- **Apuesta:** `α_j = mín(1,25 %, W/(1 + W))`. El pre-registro puede apostar menos, nunca más. Cada hipótesis **paga `α_j/(1 − α_j)` al congelarse**, pase o no, y aunque después no se corra.
- **Premio:** `ω = 5 %`, que se suma a `W` **solo si el fuera de muestra confirma** la hipótesis (una fila nueva en el libro, con el enlace a la confirmación).
- **Lo que no vota** (descriptivos, controles sin voto) no gasta.
- **Antes de congelar, el filtro de borde bruto:** `E_bruto ≥ k × C` con k = 2, costo `C` con la fórmula de §3.2 de la regla y solo valores de `config/`; estimación previa de la literatura (con cita) o `IC × σ_h` con IC de la literatura o 0,05 sin cita (§3.3). Si no lo cumple, la hipótesis no se congela ni paga, y queda anotada como "descartada por el filtro".
- **Auditoría BH/BY** sobre todo el registro en cada informe, **sin voto**.
- **Sin épocas nuevas** sin una decisión registrada del dueño en un Issue, con el motivo.
- **N del Sharpe deflactado:** base 344 (`dsr_trials_base`) + filas de ML + hipótesis que votan (§4.1 de la regla).

**Qué tiene que traer cada pre-registro nuevo:** `W` antes y después (de este libro), su `α_j`, el filtro de borde bruto con sus números y el N al congelar. El PR que lo congela agrega su fila acá. El CI (`tools/research/check_dsr_trials.py`, job `registro (dsr_trials)`) verifica la cuenta: `W` nunca negativo, cada apuesta dentro de la regla, el costo y el `W` de cada fila, y que la cadena no se corte.

Formato: porcentajes con 4 decimales; "Premio" es "—" salvo una confirmación fuera de muestra. Las filas no se editan ni se borran.

<!-- libro-alfa:inicio -->
| Fecha | Hipótesis | Pre-registro / Issue | α_j | Costo α_j/(1 − α_j) | Premio | W antes | W después |
|---|---|---|---|---|---|---|---|
| 2026-11-25 | W(0): inicio de la época (α = 5 %, η = 0,95) | #660, #690 | — | — | — | — | 4,7500 % |
<!-- libro-alfa:fin -->

Sin descubrimientos, alcanza para unas 4 hipótesis a 1,25 % (4,75 % → 3,48 % → 2,22 % → 0,95 % → la 4.ª apuesta 0,94 % y deja 0).

## Resumen al 24-11-2026

- **Estrategia en uso (15 minutos):** pierde en US500, US100, US30, US2000 y DE40. Antes de costos queda cerca de cero; los costos hacen la pérdida (R001–R009).
- **ML:** ningún modelo supera a la tabla base B1 (R010–R013).
- **Investigación:** 6 ideas probadas con reglas fijadas antes, y ninguna pasa (R014–R017, R019–R020).
- **Programa Nasdaq (#526):** H1, deriva antes del comunicado del FOMC en US100, no pasa (R027; R028 descriptivo en US500 y US30). H2, días de CPI y empleo en US100, no pasa (R031; R032 descriptivo en US500 y US30). El programa no tiene hipótesis vivas.
- **Programa 4 (#561):** estrategia A de Gianluca Bruni en US100, literal (H4.1, 15 puntos) y normalizada (H4.2, 0,35 × ATR): ninguna pasa (R029, R030). H4.3, estrategia C con perfil de volumen, tampoco (R033: pierde y no llega a 100 operaciones). El programa no tiene hipótesis vivas; la reserva sigue sin usar.
- **Programa 5 (#649):** reacción al primer toque de un FVG en US100 y US500. H5.1 (¿rebota más que un precio cualquiera?) no pasa: reacciona un poco menos que el placebo (R035; R034 fue una corrida con un error de código; R036 descriptivo en 1 hora). H5.2 (¿separa operaciones mejores de la estrategia en uso?) no pasa (R037; R038 descriptivo de la estrategia filtrada). El programa no tiene hipótesis vivas.
- **Referencia:** comprar y mantener los tres índices con los costos del CFD dio +1,10 % por mes (R018). Ninguna idea lo superó.
- **Ejemplo de comparación:** con 8 ventanas, US100 parecía estar en cero (R006). Con la historia completa, pierde −6,1 % por ventana (R007, **peor**): las 8 ventanas eran un tramo favorable.
