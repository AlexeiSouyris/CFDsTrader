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
- Tope de riesgo abierto por grupo correlacionado (achica el tamaño o rechaza con `group_risk_limit`) y colchón por gaps en el tamaño (`gap_atr`), los dos apagados en la config hasta que el dueño apruebe valores (#670, `docs/analysis/2026-11-riesgo-propuesta.md`)

## Estado

- límites y tamaño (#49)
- grupos correlacionados y gaps: mecánica con tests, config apagada (#670)
