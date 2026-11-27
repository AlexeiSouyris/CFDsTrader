# ML v2: pre-registro de las variantes y de la regla de comparaciones múltiples (#432)

> **Escrito y commiteado antes de correr ninguna variante** (24-11-2026). Sigue a la evaluación de
> ML v1 en US500 (#400, PR #430). En esa evaluación M1 no mejoró a B1: AUC 0,525, diferencia de
> log loss −0,0006 con IC de −0,0046 a 0,0039.
> Cambiar algo de este documento después de mirar un resultado invalida la comparación: se
> registra como una familia nueva, con su propia corrección.

## 1. Qué se prueba

Se agregan variables nuevas a las 24 de ML v1 (spec `docs/spec/ml-v1.md` §3). Cada grupo nuevo es
una **variante** de M1, y la variante es la **misma regresión logística L2 en PyTorch**: mismos
hiperparámetros (`config/ml.toml`), misma semilla, mismos cortes walk-forward con purga y embargo
(`config/data_split.toml`, `config/backtest.toml`). Lo único que cambia son las columnas.

| Variante | Variables nuevas | De dónde sale (solo lo publicado al cierre de la vela) |
|---|---|---|
| `tasas` | rendimiento del Tesoro de EE. UU. a 3 meses, 2 años y 10 años, en % | `treasury_yields` (ADR 0012): el último valor con `published_at` ≤ `signal_at` |
| `vix` | cierre del VIX | `vix_daily` (ADR 0012 §2): el último valor con `published_at` ≤ `signal_at` |
| `analogos` | entre los vecinos de "todos" con ATR (`config/analogs.toml`): en un largo, la proporción que tocó +2R antes que −1R (`up`); en un corto, la que tocó −1R antes que +2R (`down`) | `cfds_intel.analogs` con las diarias de futuros (todas las `.FUT`, como el informe de análogos), con la ventana que termina en la última diaria cerrada al `signal_at`. Con menos vecinos que `min_neighbors`, falta |

Las tres variantes forman **la familia**: K = 3. El orden de prioridad del Issue (sentimiento,
tasas, análogos, noticias) cambia solo por disponibilidad de datos:

- **Índice de sentimiento: no se puede evaluar.** Su historia empieza el 31-10-2025, dentro del
  tramo fuera de muestra (desde 01-11-2025), así que ninguna ventana de entrenamiento lo tiene. No
  entra a la familia.
- **Noticias: no se pueden evaluar.** No hay histórico antes de 2026 (77 interpretaciones, desde
  06-2026). No entran a la familia.

Si más adelante se suman (con más historia), se registran como una **familia nueva**, con su
propia corrección, y este pre-registro no se toca.

**Reglas de las variables:**

- **Solo información publicada.** Se usa el último valor con `published_at` ≤ `signal_at`, que es
  el cierre de la vela de la propuesta. En el histórico, el Tesoro y el VIX valen recién desde el
  día hábil siguiente a las 09:30 ET (`config/treasury.toml` y `config/vix.toml`).
- **Valor viejo = faltante.** Si el último valor publicado tiene más de `max_age_days` días
  (`config/ml.toml`, 7), la variable falta.
- **Sin relleno.** Una propuesta a la que le falta alguna variable de la variante **no entra** y
  se cuenta aparte (spec §3).
- **Python no recalcula indicadores** (CLAUDE.md §1). Las tasas y el VIX son datos publicados que
  se leen tal cual. Los análogos son de Python (su único dueño).

## 2. Cómo se decide (regla fijada antes de mirar)

- **Rival:** B1, la tabla de §13, como en la spec §6. Cada variante se compara con B1 **en sus
  mismas propuestas de validación**, que son las que tienen todas sus variables.
- **Métrica:** log loss de todas las ventanas de validación juntas. La diferencia es variante − B1.
- **Corrección por comparaciones múltiples: Bonferroni con K = 3 y error de familia del 5 %.** Una
  variante mejora a B1 si su log loss es menor y el intervalo bootstrap de la diferencia (por
  bloques de una semana ISO, 1.000 remuestreos, semilla 42) **al 1 − 0,05/3 ≈ 98,33 %** queda
  entero por debajo de 0. Para K = 1 es la regla de v1 (95 %).
- **Una sola corrida por variante**, con los datos que haya en la base el día de la corrida. Cada
  corrida se registra en `ml_experiments`, también las que no mejoran. Su `confidence` es la
  corregida (0,98333…).
- **Referencias que no deciden:** B0 y M1 sin la variante sobre las mismas propuestas, AUC, Brier y
  calibración por deciles.
- **Si ninguna variante mejora, se queda B1.** No se prueban combinaciones ni otros tenores u
  horizontes "para ver si da": eso sería otra familia.
- **Si alguna mejora**, no se usa: queda para la evaluación única fuera de muestra (spec §5, fase 5)
  y para un voto (spec §15b). **El modelo sigue siendo informativo.**

## 3. Qué no se toca

- El tramo fuera de muestra (últimos 12 meses, desde 01-11-2025): ni se entrena, ni se elige
  nada, ni se mira.
- Los mínimos por ventana (300 propuestas y 50 por clase), la grilla de L2, el corte interno del
  20 % y el embargo.

## 4. Datos disponibles al pre-registrar (solo lectura, 24-11-2026)

| Tabla | Desde | Hasta |
|---|---|---|
| `ml_labels` (corrida `ml-us500-fut-2011-v2`) | 2011 | 2026-11 |
| `ml_features` (US500.FUT) | 2014-02-04 | 2026-11-20 |
| `treasury_yields` (3M, 2Y, 10Y) | 2019-01-02 | 2026-11-20 |
| `vix_daily` | 2019-01-02 | 2026-11-20 |
| `sentiment_index` (US500.FUT) | 2025-10-31 | 2026-11-20 |

Con el Tesoro y el VIX desde 2019, las variantes `tasas` y `vix` empiezan a validar en 2021, unas
19 ventanas. Cargar el histórico desde 2011 es una decisión aparte del dueño. Si se carga después
de la corrida, una nueva corrida sería otra mirada: se registra como familia nueva.
