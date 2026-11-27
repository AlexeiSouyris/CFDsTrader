---
modulo: treasury
ruta: `intelligence/cfds_intel/treasury`
lenguaje: Python
estado: hecho
resumen: Curva par diaria del Tesoro de EE. UU. (ADR 0012) con su hora de publicación sin mirar el futuro.
---

# treasury

## Responsabilidad (única)

- Curva par diaria del Tesoro de EE. UU. (ADR 0012, #260): CSV oficial por año de home.treasury.gov (`config/treasury.toml`), con `published_at` sin mirar el futuro (día hábil siguiente a las 09:30 ET, o la hora de lectura del último valor), guardada en `treasury_yields` (solo agregar, `treasury_yield.v1`) con `python -m cfds_intel.treasury`. Contexto macro: no suma al puntaje

## Estado

- curva desde 2019 (#260)
