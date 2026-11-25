# Herramientas de verificación (ROADMAP 3b)

Leen **solo** la exportación del motor (`contracts/chart_export.v1`). No recalculan nada que use el sistema: verifican o muestran. Nada de acá alimenta señales, estrategias ni órdenes. Por eso no rompen la regla de "cada cálculo en un solo lugar" (CLAUDE.md §1): son un oráculo de prueba, no una segunda implementación.

| Herramienta | Qué hace |
|---|---|
| `compare_reference.py` | Compara SMA(50) y ATR(14) de Wilder del motor con TA-Lib, una implementación de referencia independiente, sobre las mismas velas |
| `make_indicator_fixture.py` | Genera `engine/crates/indicators/tests/fixtures/reference_indicators.csv`: velas **sintéticas** (se pueden commitear) con SMA, EMA y ATR según las fórmulas publicadas de TradingView (aritmética exacta) y según TA-Lib. El test `reference.rs` del motor lo compara en el CI (#666). Para la exportación real de TradingView, ver `docs/guides/golden-tradingview.md` |
| `viewer.html` | Visor para personas: velas, SMA, pivotes confirmados, entradas y salidas; la operación elegida muestra su stop, su TP1 y una tabla con los indicadores en la vela de la señal. Solo dibuja la exportación: no calcula nada (los números se redondean para mostrarlos y el valor completo queda al pasar el mouse) |

## Cómo se usa

```bash
# 1. Velas reales en PostgreSQL (ver engine/crates/ig/examples/ig_backfill.rs)
# 2. Exportar (el archivo tiene precios de IG: NO se commitea, el repo es público)
DATABASE_URL=... cargo run -p cfds-backtest --example chart_export -- DE40 2026-11-09T00:00:00Z 2026-11-21T00:00:00Z 10000 /tmp/de40.json
# 3. Comparar
uv run --no-project --with ta-lib==0.8.3 --with numpy python tools/verify/compare_reference.py /tmp/de40.json
# 4. Mirar: abrir tools/verify/viewer.html en el navegador y cargar /tmp/de40.json.
#    Si hay un chart_export.json al lado de la página (servida por HTTP), lo carga solo.
```

El visor usa `lightweight-charts@4.2.0` desde jsDelivr (versión fija). Horas en UTC.

Resultados documentados en `docs/research/fase-3b-validacion-referencia.md`.
