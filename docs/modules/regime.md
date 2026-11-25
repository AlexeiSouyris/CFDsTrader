---
modulo: regime
ruta: `engine/crates/regime` (crate `cfds-regime`)
lenguaje: Rust
estado: en curso
resumen: Régimen de mercado por instrumento y estados emocionales que salen del precio (spec §9.1).
---

# regime

## Responsabilidad (única)

- Clasificar el régimen de mercado por instrumento (volatilidad × tendencia) con velas cerradas
- elegir el juego de parámetros votado para ese régimen
- estados emocionales que salen del precio (spec §9.1). Hoy: FOMO de precio y pánico o clímax de venta en velas de 1h (`price_emotions`, `config/regime.toml`), también con los valores de su regla (`price_emotions_with_values`, #220). La distribución queda pendiente de volumen real. Régimen del VIX publicado (calma, normal, elevado, estrés; `vix`, umbrales en `config/analysis.toml`, #491), solo contexto

## Estado

- régimen planeado ([ADR 0003](../decisions/0003-modelo-adaptativo-y-contexto.md), aceptado, #19)
- FOMO y clímax (#220)
- régimen del VIX (#491)
