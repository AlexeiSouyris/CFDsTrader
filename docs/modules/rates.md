---
modulo: rates
ruta: `intelligence/cfds_intel/rates`
lenguaje: Python
estado: hecho
resumen: Tipos de referencia diarios de la financiación nocturna (€STR, EFFR, SOFR) y tipo de cambio EUR/USD.
---

# rates

## Responsabilidad (única)

- Tipos de referencia diarios de la financiación nocturna (#79). Hoy: €STR desde la API de datos del BCE (`config/reference_rates.toml`), con su hora de publicación real, guardado en `reference_rates` (solo agregar, `reference_rate.v1`) con `python -m cfds_intel.rates`. El motor lo usa para el costo de cada noche. EFFR y SOFR de la NY Fed (`cfds_intel.rates.nyfed`, `[nyfed]` del mismo config, ADR 0015, #467), con el mismo comando: la tasa de la base de los futuros de CME, cada valor desde las 15:00 ET de su publicación. Tipo de cambio EUR/USD diario del BCE (`config/fx_rates.toml`, `fx_rates`, `fx_rate.v1`) con `python -m cfds_intel.rates.fx`, para convertir montos del oro en USD a la cuenta en EUR (ADR 0013 §4)

## Estado

- €STR (#79)
- EUR/USD (#418)
- EFFR y SOFR (#467)
