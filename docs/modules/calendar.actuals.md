---
modulo: calendar.actuals
ruta: `intelligence/cfds_intel/calendar/actuals`
lenguaje: Python
estado: hecho
resumen: Dato real oficial del CPI y del empleo por la API pública v1 del BLS (ADR 0018 §2).
---

# calendar.actuals

## Responsabilidad (única)

- Leer de la **API pública v1 del BLS** (sin registro, User-Agent propio sin datos personales) el dato real de las series de `config/calendar_surprise.toml` ([[bls.series]]) y guardarlo en `release_actuals` (`release_actual.v1`, solo agregar). Comando `python -m cfds_intel.calendar.actuals` (una vez por día hábil, en el cron del Mac mini).
- **Una consulta por publicación**, el día hábil siguiente (hora del Este), solo de las publicaciones de `official_releases` de los últimos `lookback_days` que todavía no tienen su primer dato. Ante un 403 o un 429 frena sin reintentar.
- `released_at` = hora oficial de `official_releases`; `retrieved_at` = la lectura; `vintage` = `first` solo si se leyó antes de la publicación siguiente de la serie. El mes anterior de la misma consulta va como `revised` (lo usa la variación).
- **No calcula la sorpresa** (es de Rust, #698). No lee el sitio web del BLS (ADR 0016).

## Estado

- hecho (#697)
