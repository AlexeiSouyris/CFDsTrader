---
modulo: analogs
ruta: `intelligence/cfds_intel/analogs`
lenguaje: Python
estado: hecho
resumen: Búsqueda de situaciones históricas similares (análogos v1 sobre velas diarias).
---

# analogs

## Responsabilidad (única)

- Búsqueda de situaciones históricas similares en otras empresas e índices. Hoy (v1, #148, spec §13): forma de las últimas 20 velas diarias de los índices (z-score + DTW), 20 vecinos sin look-ahead ni episodios repetidos, retornos a 1/5/20 sesiones, +2R antes que −1R y contexto de volatilidad y tasas
- lee `daily_indicators` y el tercio de la volatilidad (`daily_vol_terciles`) de Rust y no recalcula indicadores. `config/analogs.toml`, `python -m cfds_intel.analogs`
- con `--guardar`, cada informe va a `analog_reports` como `analog_report.v1` (solo agregar, #294) para que el gateway y la UI lo lean

## Estado

- v1 sobre diarias (#148)
- informes guardados (#294)
