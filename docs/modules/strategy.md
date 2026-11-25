---
modulo: strategy
ruta: `engine/crates/strategy` (crate `cfds-strategy`)
lenguaje: Rust
estado: hecho
resumen: Combina features y señales de ML/noticias en una señal con probabilidad (puntaje de confluencia y stop estructural).
---

# strategy

## Responsabilidad (única)

- Combina features + señales de ML/noticias → señal con probabilidad. Hoy: puntaje de confluencia (§6), stop estructural (§8) y el armado de la evidencia vela por vela con alineación 15m/1h, con el calentamiento de la spec §3 que se reinicia después de un error de datos
- ajuste de noticias del puntaje y alerta de salida de la spec §9 desde `news_interpretation.v1` (`news.rs`, #130), con la fecha de corte del modelo intérprete para el backtest (`InterpreterCutoff`, `model.training_cutoff` de `config/news_interpreter.toml`, ADR 0008 §5)
- funciones puras con config en `config/strategy.toml` 
- estados emocionales de precio juntos para guardar (`price_emotional_states`: FOMO y clímax de `regime`, rechazo de `liquidity` en 15m y 1h, `emotional_state.v1`, #220)

## Estado

- puntaje y stop (#45), evidencia multi-timeframe (#47), calentamiento (#88), errores de datos (#89), ajuste de noticias (#130), corte del intérprete (#194)
