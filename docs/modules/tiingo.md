---
modulo: tiingo
ruta: `engine/crates/tiingo` (crate `cfds-tiingo`)
lenguaje: Rust
estado: hecho
resumen: Verificación transitoria de las velas de US500, US100 y US30 contra Tiingo (ADR 0009).
---

# tiingo

## Responsabilidad (única)

- Verificación **transitoria** de nuestras velas de US500, US100 y US30 contra SPY, QQQ y DIA de Tiingo, plan gratis (ADR 0009, opción B): compara variaciones diarias en memoria y guarda solo el veredicto (`tiingo_verifications`)
- topes con ventanas móviles y registro de uso sin precios (`tiingo_usage_log`), kill switch y solo con `CFDS_DATA_MACHINE=1`
- `config/tiingo.toml` y el ejemplo `tiingo_verify`

## Estado

- hecho (#206)
