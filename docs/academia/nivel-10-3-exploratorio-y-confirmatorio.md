# 10.3 Exploratorio y confirmatorio en el Laboratorio

> **Nivel 10 · Tu propia estrategia.** Educativo: no es consejo de inversión ni promete ganancias.
> Todo pasa en simulación ([paper trading](../GLOSARIO.md#paper-trading)). El Laboratorio corre el
> pasado con velas guardadas: no manda órdenes de ningún tipo.

## Objetivo

Saber qué se puede hacer en el Laboratorio **sin gastar presupuesto** (explorar: no vota) y qué
hace falta para que una idea **vote** (confirmar, con un pre-registro congelado).

## La idea

El Laboratorio arma una estrategia con bloques, la corre en el motor con las velas guardadas y da
un informe honesto. Tiene dos modos, y la diferencia es todo:

| | Exploratorio | Confirmatorio |
|---|---|---|
| Para qué | probar ideas, aprender, descartar | votar una hipótesis |
| Qué pide | nada | un pre-registro congelado, con hash, en el libro del presupuesto |
| Etiqueta del informe | **«exploratorio, no vota»** | el criterio pre-registrado da el veredicto |
| Alfa | no gasta | paga α_j ÷ (1 − α_j) al congelarse |
| Fuera de muestra | nunca | una sola vez por hash, solo si pasó |
| En la app hoy | **sí** | **todavía no** |

**1. Explorar está bien, si se cuenta.** Mirar 23 variantes de una idea es normal; lo que no está
bien es mostrar solo la mejor. Por eso el Laboratorio:

- guarda cada variante probada (de solo agregar); una spec repetida no suma de nuevo;
- agrupa las variantes en **familias** (mismo instrumento y mismos tipos de bloque; la definición
  está pendiente del dueño);
- usa en el Sharpe deflactado N = N del proyecto + variantes distintas de la familia, nunca menos
  que el del proyecto (R040: 361 + 4 = 365);
- calcula el **PBO** de la familia desde 4 variantes;
- acepta como mucho **20 variantes** por corrida y no tiene un botón «optimizar» que busque la
  mejor combinación.

**2. Lo que el Laboratorio nunca deja hacer:**

- arriesgar más que `config/risk.toml` por operación;
- bajar los costos: solo se suben (multiplicador desde 1 hasta 5), y cada informe trae además la
  versión con costos × 1,5;
- tocar el fuera de muestra: la corrida se rechaza;
- usar más de 3 condiciones de entrada o más de 2 filtros.

**3. Qué trae cada informe** (todo lo calcula el motor): curva y caída con comprar y mantener
encima, distribución de R antes y después de costos, expectativa con su intervalo, Sharpe y Sharpe
deflactado con su N, PBO, Monte Carlo, walk-forward por ventana, las dos mitades, costos × 1,5, el
test de suerte, MAE/MFE y el **borde bruto medido contra k × C**. En exploratorio, ese borde bruto
es un resultado, no una estimación previa: no vale para el filtro de la lección 10.2.

**4. Cómo se confirma hoy.** El modo confirmatorio del Laboratorio todavía no existe: el motor
rechaza una spec confirmatoria («el confirmatorio llega con el pre-registro»). Mientras tanto,
**congelar es un PR**: el pre-registro, su `.sha256` y la fila del libro del presupuesto, que
mergea el dueño o su agente con el CI en verde. Cuando llegue, el motor va a correr una
confirmatoria solo si el SHA-256 del pre-registro coincide y su fila está en el libro de `main`.

**5. El camino completo de una idea propia:** explorar en el Laboratorio (sin gastar) → si algo
parece valer, escribir la hipótesis completa (10.1) → pasar el filtro de borde bruto con una
estimación **previa**, no con lo que viste explorando (10.2) → congelar y pagar → correr una vez →
si pasa, una sola mirada al fuera de muestra. Lo más probable, como en nuestro registro, es que la
respuesta sea «no»; saberlo en simulación es barato.

> **Probá antes de seguir:** exploraste 15 variantes de un cruce de medias y la mejor da +0,12 R
> por operación. ¿Podés usar ese +0,12 R como estimación previa para el filtro de borde bruto?
>
> <details><summary>Ver la respuesta</summary>
>
> No. Es un resultado de la exploración, elegido entre 15, y además está sobre los mismos datos que
> después vas a usar para confirmar. La estimación previa tiene que venir de la literatura (con
> cita) o de IC × σ_h, sin mirar el resultado de la regla.
>
> </details>

## Interactivo

El constructor del Laboratorio: armar una spec con bloques, validarla (el motor dice qué está mal
y dónde), correrla y leer el informe con su etiqueta «exploratorio, no vota».

## Ejercicio

Armá en papel una familia de 4 variantes de un cruce de medias (cambia solo la media rápida). Antes
de correrla: ¿qué N va a usar el Sharpe deflactado? ¿Qué te diría un PBO de 0,8?

<details><summary>Ver la solución</summary>

N = N del proyecto + 4 (hoy 361 + 4 = 365, como R040). Un PBO de 0,8 diría que, en el 80 % de las
particiones, la mejor variante del entrenamiento quedó en la mitad de abajo al probarla: elegir «la
mejor» de esa familia no sirve.

</details>

## Examen

5 preguntas. Para aprobar hace falta el 80 % (umbral provisorio).

## Desbloquea

- **Laboratorio**: el constructor de estrategias, las corridas y los informes, en modo
  exploratorio.

## Cifras y fuentes

| Cifra | Valor | Fuente | Prueba |
|---|---|---|---|
| Etiqueta de los informes exploratorios | `exploratorio, no vota` | `engine/crates/backtest/src/lab/report.rs` | A |
| El motor rechaza una spec confirmatoria | `el confirmatorio llega con el pre-registro (L13)` | `engine/crates/backtest/src/lab/validate.rs` | A |
| En la v1, congelar es un PR | `congelar es un PR` | `docs/design/2026-11-laboratorio.md` | A |
| Alfa: el confirmatorio paga al congelarse | `aunque no se corra` | `docs/design/2026-11-laboratorio.md` | A |
| Fuera de muestra: una sola vez por hash | `puede evaluarse en el fuera de muestra` | `docs/design/2026-11-laboratorio.md` | A |
| Sin botón «optimizar» | `No tiene botón "optimizar"` | `docs/design/2026-11-laboratorio.md` | A |
| N del Sharpe deflactado del Laboratorio | `N = N del proyecto + variantes de la familia` | `docs/modules/lab.md` | A |
| R040 con N 365 | `Sharpe deflactado con N 365` | `docs/analysis/registro-de-resultados.csv` `R040` | A |
| PBO desde 4 variantes | `PBO por CSCV desde 4 variantes` | `docs/modules/lab.md` | A |
| Variantes por corrida | 20 (`20`) | `config/lab.toml` `limits.max_variants_per_run` | A |
| Multiplicador de costos máximo | 5 (`5`) | `config/lab.toml` `limits.max_cost_multiplier` | A |
| Estrés de costos de cada informe | `costos × 1,5` | `docs/design/2026-11-laboratorio.md` | A |
| Condiciones de entrada como máximo | 3 (`3`) | `config/lab.toml` `limits.max_entry_conditions` | A |
| Filtros como máximo | 2 (`2`) | `config/lab.toml` `limits.max_filters` | A |
| Definición de familia, pendiente | `definición PENDIENTE del dueño` | `docs/modules/lab.md` | A |
