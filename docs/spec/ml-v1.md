# Spec del modelo de ML v1 (fase 4b): BORRADOR

> **Estado: borrador para que lo apruebe el dueño (#158).** Se escribe **antes de mirar resultados**. Nada de esto se programa hasta que el dueño lo apruebe en el Issue (modo de un solo constructor, CLAUDE.md). Las decisiones abiertas están al final, cada una con una recomendación.

## 1. Qué predice

**La probabilidad de que una propuesta toque TP1 antes que el stop.** Es la misma pregunta de la spec §13, ahora con un modelo en vez de una tabla de frecuencias.

- **Unidad:** cada propuesta que arma el motor en el backtest (`proposal.v1`): puntaje ≥ 5 y todos los filtros de §7 superados, al cierre de la vela de 15m `t`, con entrada en la apertura de `t+1`.
- **Etiqueta:** `y = 1` si `outcome.v1.hit_tp1` es verdadero, y `y = 0` si no lo es. Como la operación termina al tocar el stop, `hit_tp1 = true` ya significa "TP1 antes que el stop". Las salidas por tiempo y los cierres forzados cuentan como `0` (decisión abierta 1).
- **Salida:** una probabilidad entre 0 y 1 **con su intervalo** (§6). Nunca una certeza (CLAUDE.md §5).

## 2. Para qué se usa en v1

**Solo informativa, igual que los análogos v1:** se muestra al lado de la probabilidad de §13 y **no suma al puntaje** ni filtra propuestas. Para que cambie una decisión del sistema, hace falta un voto (spec §15b) después de que mejore fuera de muestra.

## 3. Variables (solo información disponible al cierre de `t`)

Todas son valores que **ya calcula Rust** o que vienen en la propuesta. Python **no recalcula indicadores** (CLAUDE.md §1): los lee de PostgreSQL.

| Grupo | Variable | De dónde sale hoy |
|---|---|---|
| Propuesta | dirección (largo/corto) | `proposal.v1.side` |
| Propuesta | instrumento (codificado one-hot) | `proposal.v1.instrument_id` |
| Propuesta | gatillos presentes (4 indicadores 0/1) | `proposal.v1.triggers` |
| Propuesta | puntos de cada componente del puntaje (9 valores) | `proposal.v1.breakdown` |
| Propuesta | puntaje total | `proposal.v1.score` |
| Riesgo | distancia al stop en ATR de 15m: `(referencia − stop) / ATR14` | `reference`, `stop` y el ATR de `indicators` |
| Riesgo | distancia a TP1 en R | `reference`, `stop` y `tp1` |
| Contexto | volatilidad realizada diaria en tercios (baja/media/alta, como en §13) | `daily_indicators` (#148) |
| Contexto | hora de la sesión en franjas (apertura, media, cierre) | `signal_at` y `config/sessions.toml` |

- **No entran en v1:** el texto de las noticias (solo su ajuste, que ya está en `breakdown.news`), los análogos, el sentimiento (COT, IG) ni las tasas. Se pueden probar en v2, cada uno como una variante del registro de experimentos (§7).
- **Faltante:** el ATR14 de 15m al cierre de `t` y la franja de sesión no viajan hoy con la propuesta. Antes de programar, se agrega un contrato `ml_features.v1` (contratos primero) que **arma Rust** con una fila por propuesta y la hora `as_of = signal_at`.
- **Calentamiento:** una propuesta sin todas sus variables (por ejemplo, sin 3 años de historia para el tercio de volatilidad) **no entra** al entrenamiento y queda contada aparte. No se rellena con promedios.

## 4. Modelos que se comparan

En orden, del más simple al más complejo. Uno más complejo solo se acepta si le gana al anterior (§6).

| Nombre | Qué es | Para qué está |
|---|---|---|
| **B0: frecuencia global** | la proporción de `y = 1` en la ventana de entrenamiento | piso: no saber nada |
| **B1: tabla de §13** | la frecuencia por tipo de gatillo × rango de puntaje, con el mismo mínimo de 30 casos (si no llega, cae a B0) | lo que se muestra hoy: **el rival a vencer** |
| **M1: regresión logística** | con penalización L2, sobre las variables de §3 estandarizadas con la media y el desvío **solo del entrenamiento**; en PyTorch, para usar desde ya la misma herramienta de los modelos siguientes | el modelo v1 |

Una red (MLP) queda para v2, y solo si M1 le gana a B1.

## 5. Cómo se divide el tiempo

Con `config/data_split.toml` tal como está, sin cambiar sus números:

- **Fuera de muestra:** los últimos 12 meses (`[out_of_sample] months = 12`) **no se tocan**: no se entrena, no se elige nada y no se mira. Se evalúan **una sola vez** en la fase 5, con el modelo ya congelado, y el resultado se informa aparte, sin retocar.
- **Walk-forward** sobre el resto: se entrena con 24 meses, se valida con los 3 siguientes y se avanza 3 (`train_months`, `validation_months`, `step_months`). Todas las métricas son del **conjunto de las ventanas de validación**, nunca de una sola.
- **Purga:** una propuesta de entrenamiento cuyo resultado se conoce **después** del inicio de su ventana de validación se saca del entrenamiento. Si no, la etiqueta filtraría información del período de validación.
- **Embargo:** después de cada ventana de validación, no se usan para entrenar las propuestas de las siguientes `time_exit_bars` velas (`config/backtest.toml`, hoy 32), que es lo máximo que dura una operación sin tocar TP1 ni el stop.
- **Hiperparámetros** (la fuerza de la L2, por ejemplo): se eligen **dentro** de cada ventana de entrenamiento, con un corte temporal interno (el último 20 %), nunca con la validación.

## 6. Cómo se mide y cuándo "mejora"

- **Métrica principal: log loss** (pérdida logarítmica) sobre todas las ventanas de validación juntas.
- **Secundarias:** puntaje de Brier, gráfico de calibración (por deciles, con su intervalo de Wilson) y AUC.
- **Criterio de mejora (fijado antes de mirar):** M1 mejora a B1 si su log loss es menor y el **intervalo de confianza del 95 % de la diferencia** (bootstrap por bloques de una semana, 1.000 remuestreos) queda entero por debajo de cero. Si no, se queda B1.
- **Intervalo de cada predicción:** el rango entre los percentiles 5 y 95 de las predicciones de 200 modelos M1 entrenados sobre remuestreos bootstrap del entrenamiento. Si hay menos de 30 propuestas parecidas (mismo gatillo y rango de puntaje) en el entrenamiento, se muestra "datos insuficientes", igual que en §13.
- **Mínimo de datos:** una ventana de entrenamiento con menos de **300 propuestas** o menos de **50 de cada clase** no entrena (decisión abierta 3). Si ninguna ventana llega, ML v1 queda en suspenso y se sigue mostrando B1.

## 7. Registro de experimentos (ADR 0003 §D)

Cada entrenamiento queda registrado, **también los que no funcionan**: fecha (UTC), versión del código, variables usadas, hiperparámetros, ventanas, métricas por ventana y el resultado contra B1. Sirve para el Sharpe deflactado y el PBO de la fase 5. Antes de programar se define su contrato (`ml_experiment.v1`) y se guarda en PostgreSQL, de solo agregar.

## 8. Tests que tienen que existir (sin fuga)

1. **Sin look-ahead en las variables:** si se cambian todas las velas posteriores a `t`, ninguna variable de la propuesta de `t` cambia. El test falla si alguna usa datos posteriores a `t`.
2. **Sin repintado:** las variables de una propuesta ya armada no cambian al llegar velas nuevas.
3. **Cortes:** ninguna propuesta aparece en entrenamiento y validación de la misma ventana; la purga y el embargo sacan exactamente las que deben; el tramo fuera de muestra nunca se lee.
4. **Estandarización:** la media y el desvío salen solo del entrenamiento.
5. **Reproducible:** con la misma semilla y los mismos datos, el mismo modelo y las mismas probabilidades.

## 9. Dónde vive cada parte

| Parte | Dónde |
|---|---|
| Propuestas, resultados y variables (`ml_features.v1`) | Rust (`backtest`, `journal`), en PostgreSQL |
| Cortes, entrenamiento, evaluación, registro de experimentos | Python (`intelligence/cfds_intel/models`) |
| Predicción para la UI (contrato `ml_prediction.v1`, a definir) | Python la publica; Rust y la UI solo la muestran |

## 10. Datos que hacen falta antes de entrenar

- **5 años de los 4 índices** (#102) para las variables diarias, y **la historia de 15m** del backtest que genera las propuestas. Hoy no alcanza: cuánto cargar lo decide el dueño (cupo de IG, solo en el Mac mini).
- Con 12 meses fuera de muestra y ventanas de 24 + 3, la primera validación necesita **al menos 27 meses de 15m antes del tramo fuera de muestra**, es decir **unos 39 meses** de historia de 15m en total.

## 11. Decisiones abiertas (para el dueño)

1. **Salidas por tiempo y cierres forzados.** *Recomendación:* que cuenten como `y = 0`, porque la pregunta es "¿toca TP1?". La alternativa es sacarlas del entrenamiento, pero eso infla la probabilidad.
2. **El modelo base en PyTorch o en una librería clásica** (scikit-learn). *Recomendación:* PyTorch, como dice el ROADMAP, para no sumar otra librería principal (que pediría un ADR).
3. **Mínimo de datos por ventana** (300 propuestas y 50 de cada clase). *Recomendación:* dejarlo así; si con los datos reales no se llega, ML v1 queda en suspenso en vez de bajar el mínimo.
4. **Cuánta historia de 15m cargar** (§10: unos 39 meses). Depende del cupo de IG y de #151.
5. **Que v1 sea solo informativa** (§2). *Recomendación:* sí, hasta que mejore fuera de muestra y se vote.

## 12. Supuestos y riesgos

- **Pocas propuestas.** Con puntaje ≥ 5 y todos los filtros, quizás haya pocas por mes. Si no se llega al mínimo de §6, el modelo no se entrena: no se bajan los requisitos para que "dé algo".
- **Correlación entre índices.** Las propuestas de US100, US500 y US30 del mismo día no son independientes. Por eso el bootstrap es por bloques de semana y no por propuesta.
- **El backtest no es el mercado.** Las etiquetas salen de la simulación (con costos, §11). En la fase 8 se comparan con el paper trading.
- **Sobreajuste por probar muchas variantes.** Lo controla el registro de experimentos (§7) y la regla de una sola evaluación fuera de muestra.
