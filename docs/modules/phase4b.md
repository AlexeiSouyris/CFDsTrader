---
modulo: phase4b
ruta: `intelligence/cfds_intel/phase4b`
lenguaje: Python
estado: en curso (#221)
resumen: Reporte de aprobación de la fase 4b: cada capa contra no usarla, sin fuga entre períodos.
---

# phase4b

## Responsabilidad (única)

- Reporte de aprobación de la fase 4b (ROADMAP): cada capa contra no usarla, sin fuga entre períodos. Noticias: dos corridas de `backtest_run` (con ajuste y `--sin-noticias`), P/L neto por semana (de `outcomes`, calculado por Rust) en las ventanas de validación walk-forward, con el mismo bootstrap por semanas de ML §6 (`models.compare.block_interval`)
- análogos: no aplica (solo informativos)
- ML: lee el resultado registrado en `ml_experiments`. Escribe el informe en markdown (`python -m cfds_intel.phase4b`, `config/phase4b.toml`)

## Estado

- en curso (#221)
