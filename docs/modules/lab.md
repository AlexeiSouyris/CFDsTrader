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
