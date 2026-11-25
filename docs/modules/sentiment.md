---
modulo: sentiment
ruta: `intelligence/cfds_intel/sentiment`
lenguaje: Python
estado: hecho
resumen: Fuentes de sentimiento con su hora de publicación real (ADR 0004): COT e índice de sentimiento.
---

# sentiment

## Responsabilidad (única)

- Fuentes de sentimiento con su hora de publicación real (ADR 0004). Hoy: COT de la CFTC (TFF, futuros) desde la API pública, guardado como observaciones crudas en `sentiment_observations` con `python -m cfds_intel.sentiment`. Índice de sentimiento v1 (spec §9.1, `index.py`, `config/sentiment.toml`): valor crudo del COT, percentiles sobre 3 años, calentamiento, vencimiento por componente y mínimo de componentes, como `sentiment.v1`, guardado en `sentiment_index` (solo agregar) con `python -m cfds_intel.sentiment.compute_index` (`index_store.py`). Estados que salen del índice (spec §9.1, `states.py`, `[states]` de `config/sentiment.toml`): miedo extremo, euforia extrema y complacencia. Arnés de las hipótesis H1–H7 (`hypotheses.py`, `config/hypotheses.toml`): bootstrap del IC 95 % de la diferencia contra la media, "datos insuficientes" con menos de 30 casos
- hoy solo con datos sintéticos. 
- los estados del índice con los valores de su regla se guardan en `emotional_states` (`states_store`, #220)

## Estado

- COT (#113)
- índice v1 (#213)
- estados del índice (#220)
- arnés de hipótesis (#227)
