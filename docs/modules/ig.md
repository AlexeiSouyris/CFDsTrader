---
modulo: ig
ruta: `engine/crates/ig` (crate `cfds-ig`)
lenguaje: Rust
estado: hecho
resumen: Proveedor de IG (cuenta demo, solo lectura): histórico y velas en vivo por REST, condiciones, spread y cupo.
---

# ig

## Responsabilidad (única)

- Proveedor de IG, cuenta **demo**, solo lectura: sesión REST e histórico de velas (precio medio exacto, UTC) detrás de `MarketDataProvider`, con límites de uso, cupo semanal y reintentos de `config/ig.toml`
- condiciones del contrato (`GET /markets/{epic}`) y spread al cierre para completar `config/instruments.toml` y para guardarlo con cada vela del mismo pedido (#95)
- velas diarias de 5 años (`daily_history`, #102)
- modo en vivo: cada vela cerrada se pide por REST apenas cierra (1 punto por vela, solo en horario de sesión) con kill switch, retomando desde la última vela guardada para correr con `--once` desde el `cron` (`live.rs`, `Recent`, #124), y guardando en `candle_spreads` el spread de cierre de cada vela cerrada del mismo pedido, sin cupo extra (`Recent::history_with_spreads`, `spreads_to_store`, #437)
- sentimiento de clientes de IG (% largos) para grabar cada N minutos (`sentiment.rs`)
- lo que dice IG del cupo en cada respuesta y su resumen: gastado, gastado por otro lado y renovación (`allowance.rs`)
- reglas antes de una carga: `--dry-run`, confirmación arriba de un umbral y tope en máquinas sin `CFDS_DATA_MACHINE=1` (`loads.rs`, `[loads]` de `config/ig.toml`)
- presupuesto semanal del cupo por uso (en vivo, historia, fuera del Mac mini y reserva), con lo gastado sacado de `ig_allowance_log` (`budget.rs`, `[budget]` de `config/ig.toml`)

## Estado

- histórico REST (#66), condiciones y spread (#67), en vivo por REST (#68), sentimiento (#27), registro del cupo (#140), dry-run y topes de carga (#143), presupuesto por uso (#167)
