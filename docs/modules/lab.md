---
modulo: lab
ruta: `engine/crates/backtest/src/lab` (módulo `cfds_backtest::lab`)
lenguaje: Rust
estado: en curso (#710)
resumen: Laboratorio de estrategias: specs por bloques (`strategy_spec.v1`), su catálogo, su validación con hash canónico y su corrida con el arnés de investigación.
---

# lab

Diseño: `docs/design/2026-11-laboratorio.md` (#710). Contratos: `strategy_catalog.v1`, `strategy_spec.v1` y `spec_validation.v1` (#725).

## Responsabilidad (única)

- Traducir una estrategia descrita con bloques (la arma la UI o un archivo JSON) a lo que ya corre el arnés de investigación (`research`), con las barandas del diseño: catálogo cerrado, riesgo nunca mayor que `config/risk.toml`, costos nunca menores que los del catálogo y fuera de muestra bloqueado.
- No calcula indicadores, riesgo, costos ni estadística propios: usa `cfds-indicators`, `cfds-risk`, `research::*` y los costos del backtest. La UI nunca calcula nada de esto.

## Estado

- Tipos de los contratos, spec validada con sus bloques tipados y hash canónico: SHA-256 de la spec con las claves ordenadas y sin espacios (`spec_sha256`) y de la familia, mismo instrumento y mismos tipos de bloque, sin orden en las condiciones de entrada y los filtros (`family_sha256`, definición PENDIENTE del dueño, §9 punto 2) (#745).
- Catálogo del motor (`blocks::catalog`, #746): los bloques v1 del código (entrada `ma_cross` y `opening_range_breakout`, filtro `entry_hours_utc`, stop `atr`, salida `time_target`) con los rangos y valores propuestos de `config/lab.toml` (nuevo; valores recomendados del diseño, PENDIENTES §9 puntos 3, 4 y 7) y el riesgo máximo y la distancia del stop en ATR de `config/risk.toml`. Falla al arrancar si la config tiene un bloque o parámetro que el código no conoce, le falta uno o un rango es inválido.
- Validación (`validate_spec`, `validate_spec_json`, #726): la spec contra el catálogo con **todos** los errores a la vez (`spec_validation.v1`: código, ruta JSON Pointer y mensaje): bloque o parámetro desconocido, en otro lugar o con otro timeframe, parámetro faltante, de otro tipo o fuera de rango, combinaciones inválidas (media rápida ≥ lenta, salida sin tiempo, pre-registro en una exploratoria), topes de condiciones y filtros, riesgo mayor que `config/risk.toml`, costos menores que los del catálogo, período inválido o que llega al fuera de muestra (guardia `research_period`) y modo confirmatorio (todavía no, L13). Si es válida devuelve la spec normalizada (decimales sin ceros de más, así `"2.0"` y `"2"` dan el mismo hash) con sus bloques tipados.
- Planes (`plans_from_spec`, `LabSeries`, #727): una spec válida → los `research::Plan` que ejecuta `run_plans`. Señal con velas cerradas del timeframe de la spec (15m o 1h; la de 1h decide en la vela de 15m que cierra con la hora), condiciones unidas con "y" (cruce de medias con `cfds_indicators::crosses`; ruptura del rango de apertura con `research::orb_plans`, la misma de la ronda 1), dirección y filtro de horas UTC; decisión permitida por las marcas de entrada y entrada en la apertura siguiente; stop a k × ATR de Wilder (el mismo ATR mide el riesgo; sin ATR no hay plan); salida planificada en la vela N y/o la última de la sesión de contado (una salida todavía desconocida o un hueco justo ahí no dan plan) y objetivo en R. Test de propiedad: cortar la serie en cualquier vela no cambia los planes anteriores.
- Secciones del informe (`sections`, #756): cada bloque del `lab_report.v1` armado con lo que ya existe (esperanza en R con IC normal y win rate con IC de Wilson, histograma de R, curva y caída, Sharpe y Sortino, cuantiles de MAE/MFE, cortes, PBO por CSCV desde 4 variantes, Monte Carlo por bloques, operaciones y rechazos, avisos fijos), con números como texto decimal y `null` cuando algo no está definido. Convenciones en `[report]` de `config/lab.toml` (nivel 0,95, tramos de 0,5 R, 1000 entradas al azar).
- Informe completo (`lab_report`, `run_setup`, #761): el recorrido de la spec con el riesgo por operación de la spec (nunca más que el de config), los retornos por operación (`trade_return`) con los costos × multiplicador y × 1,5, los meses (`monthly_returns`), la curva con comprar y mantener, Sharpe, Sortino y Sharpe deflactado con N = N del proyecto + variantes de la familia (nunca menos que el del proyecto), mitades, walk-forward por ventana, test de suerte, MAE/MFE, borde bruto medido contra `k × C` y cortes; `report_id` = SHA-256 de spec, config, commit, última vela y semilla. Siempre "exploratorio, no vota".
- Ensayos (`trial`, `planned_exit_for`, #764): `lab_trial.v1` de solo agregar en un archivo JSON Lines (hasta la tabla de L8) y el conteo de variantes distintas de la familia para el N del Sharpe deflactado del laboratorio (una spec repetida no suma de nuevo; nunca al N del proyecto); la salida planificada de la spec para cualquier decisión, para que las entradas al azar del test de suerte salgan igual que la estrategia.
- Ejemplo `laboratorio` (#730), **primer valor sin UI**: valida una spec o varias variantes de una familia contra el catálogo (`--validar` solo valida), carga las velas de futuros solo hasta el inicio del fuera de muestra (como las rondas, en una sesión de solo lectura, nunca IG), corre cada variante, arma el test de suerte con la misma regla de salida, comprar y mantener y el `lab_report.v1`, escribe los informes en `--salida` y agrega los ensayos a `--ensayos`. Exploratorio: no vota.

  ```text
  DATABASE_URL=...?options=-c%20default_transaction_read_only%3Don \
  cargo run --release -p cfds-backtest --example laboratorio -- \
      spec.json [variante.json ...] --escenario-financiacion ESTR --ensayos ensayos.jsonl \
      [--supuestos archivo.toml] [--capital 100000] [--desde 2011-01-01T00:00:00Z] \
      [--salida carpeta] [--validar]
  ```
