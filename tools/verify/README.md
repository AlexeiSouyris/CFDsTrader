# Herramientas de verificación (ROADMAP 3b)

Leen **solo** la exportación del motor (`contracts/chart_export.v1`). No recalculan nada que use el sistema: verifican o muestran. Nada de acá alimenta señales, estrategias ni órdenes. Por eso no rompen la regla de "cada cálculo en un solo lugar" (CLAUDE.md §1): son un oráculo de prueba, no una segunda implementación.

| Herramienta | Qué hace |
|---|---|
| `compare_reference.py` | Compara SMA(50) y ATR(14) de Wilder del motor con TA-Lib, una implementación de referencia independiente, sobre las mismas velas |

## Cómo se usa

```bash
# 1. Velas reales en PostgreSQL (ver engine/crates/ig/examples/ig_backfill.rs)
# 2. Exportar (el archivo tiene precios de IG: NO se commitea, el repo es público)
DATABASE_URL=... cargo run -p cfds-backtest --example chart_export -- DE40 2026-11-09T00:00:00Z 2026-11-21T00:00:00Z 10000 /tmp/de40.json
# 3. Comparar
uv run --no-project --with ta-lib==0.8.3 --with numpy python tools/verify/compare_reference.py /tmp/de40.json
```

Resultados documentados en `docs/research/fase-3b-validacion-referencia.md`.
