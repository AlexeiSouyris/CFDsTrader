---
modulo: vix
ruta: `intelligence/cfds_intel/vix`
lenguaje: Python
estado: hecho
resumen: VIX diario desde el CSV oficial de Cboe (ADR 0012 §2).
---

# vix

## Responsabilidad (única)

- VIX diario (ADR 0012 §2, #260), por decisión del dueño y contra los términos de Cboe: una lectura del CSV oficial de cboe.com por día hábil (`config/vix.toml`), que se frena ante un 403 o un desafío anti-bots. Va a `vix_daily` (solo agregar, `vix_daily.v1`) con `python -m cfds_intel.vix`. Los datos viven solo en la base del Mac, nunca en el repo. `cfds_intel.daily_publication` tiene la regla de `published_at` que comparte con `treasury`. Contexto: no suma al puntaje

## Estado

- desde 2019 (#260)
