---
modulo: risk
ruta: `engine/crates/risk` (crate `cfds-risk`)
lenguaje: Rust
estado: hecho
resumen: Apalancamiento por instrumento, tamaño de posición, margen, stop-loss y pérdida máxima.
---

# risk

## Responsabilidad (única)

- Apalancamiento por instrumento, tamaño de posición, margen, stop-loss, pérdida máxima. Hoy: `check` (límites de la cuenta §10, distancia al stop §7.7, tamaño, tope de apalancamiento, margen y mínimo del broker §8), con config en `config/risk.toml`

## Estado

- límites y tamaño (#49)
