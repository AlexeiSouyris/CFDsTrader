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
- Tope de riesgo abierto por grupo correlacionado (achica el tamaño o rechaza con `group_risk_limit`) y colchón por gaps en el tamaño (`gap_atr`) (#670, `docs/analysis/2026-11-riesgo-propuesta.md`)
- Reducción gradual del riesgo por caída (`[drawdown_scaling]`, lineal o por escalones) y volatilidad objetivo de la cuenta (`[vol_target]`) (#671)

## Valores en `config/risk.toml` (aprobados por el dueño el 25-11-2026, #689)

- Grupo `us_equity_index` (US100, US500, US30, US2000) con `max_risk_pct = "1.5"`. DE40 y XAUUSD sin grupo.
- `gap_atr = "0.25"` (provisional: se ajusta con el deslizamiento real más allá del stop que mida el paper).
- Caída gradual lineal: (5 %, 1), (10 %, 0,5), (12,5 %, 0,25). El kill switch del 15 % no cambia.
- Volatilidad objetivo **apagada**: nadie calcula todavía la volatilidad realizada de la cuenta (#692).
- El runner de paper (#382) usa estos valores desde que arranca.
- Tests: la mecánica se prueba sobre una config neutral (`engine/crates/risk/tests/neutral_risk.toml`, todo apagado); `unchanged.rs` prueba que esa config da lo mismo que el cálculo anterior a #670, y `the_repository_config_matches_the_spec` fija los valores del repositorio.
- Pendiente conocido (#691): el broker simulado actualiza el máximo del capital solo al decidir y el backtest en cada vela, así que con la caída gradual los tamaños de paper y backtest pueden separarse.

## Estado

- límites y tamaño (#49)
- grupos correlacionados y gaps: mecánica con tests (#670); valores aprobados en la config (#689)
- caída gradual: mecánica con tests (#671); valores aprobados en la config (#689)
- volatilidad objetivo: mecánica con tests, config apagada; falta conectar la volatilidad realizada (#671)
