---
modulo: research-checks
ruta: `tools/research/`
lenguaje: Python (solo biblioteca estándar)
estado: en curso (#660)
resumen: Chequeos de CI de las reglas de investigación: el último `dsr_trials` cubre las pruebas del registro de resultados.
---

# research-checks

## Responsabilidad (única)

- `check_dsr_trials.py` verifica que el último `dsr_trials` de `config/research.toml` cubra las pruebas de `docs/analysis/registro-de-resultados.csv`, con la regla de conteo de `docs/analysis/2026-11-regla-presupuesto-global.md` §4 (propuesta, pendiente de aprobación del dueño). Falla si el registro tiene más pruebas que el N o si un `dsr_trials` baja.
- No calcula nada del sistema: solo cuenta filas y compara con la config.

## Estado

- Propuesta y chequeo en el CI (job `registro (dsr_trials)`), #660.
