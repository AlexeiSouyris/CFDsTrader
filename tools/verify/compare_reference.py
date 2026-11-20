"""Compara los indicadores del motor con una implementación de referencia independiente (TA-Lib).

Lee SOLO la exportación del motor (contracts/chart_export.v1, #81): OHLC y los valores del motor.
TA-Lib calcula sus propios valores con las mismas velas y se informa la diferencia máxima.
Es una herramienta de verificación (ROADMAP 3b), no parte del sistema: nada de esto alimenta
señales ni estrategias.

    uv run --no-project --with ta-lib==0.8.3 --with numpy python tools/verify/compare_reference.py export.json

- SMA(50): TA-Lib usa la misma definición (media de los últimos 50 cierres): tiene que ser igual.
- ATR(14) de Wilder: el motor sigue la convención de TradingView (primer TR = H − L y el primer
  ATR es el promedio de los primeros 14 TR); TA-Lib arranca un paso después. Con el suavizado de
  Wilder la diferencia inicial se multiplica por 13/14 en cada vela, así que se compara desde que
  la diferencia inicial ya es despreciable (se informa también la máxima del arranque).
"""

import json
import sys
from decimal import Decimal

import numpy as np
import talib


def main(path: str, warmup_atr: int = 300) -> int:
    with open(path) as f:
        export = json.load(f)
    candles = export["candles"]
    high = np.array([float(c["high"]) for c in candles])
    low = np.array([float(c["low"]) for c in candles])
    close = np.array([float(c["close"]) for c in candles])
    ref_sma = talib.SMA(close, timeperiod=50)
    ref_atr = talib.ATR(high, low, close, timeperiod=14)

    def compare(name: str, key: str, ref: np.ndarray, start: int) -> float:
        worst = 0.0
        for i, c in enumerate(candles):
            mine = c[key]
            if i < start or mine is None or np.isnan(ref[i]):
                continue
            diff = abs(float(Decimal(mine)) - ref[i])
            worst = max(worst, diff / max(abs(ref[i]), 1e-12))
        print(f"{name}: diferencia relativa máxima desde la vela {start}: {worst:.3e}")
        return worst

    n = len(candles)
    print(f"{export['instrument_id']} {export['timeframe']}: {n} velas")
    sma_first = next(i for i, c in enumerate(candles) if c["sma"] is not None)
    print(f"SMA: primer valor del motor en la vela {sma_first} (TA-Lib: 49)")
    sma = compare("SMA(50)", "sma", ref_sma, 0)
    compare("ATR(14) arranque", "atr", ref_atr, 0)
    atr = compare("ATR(14) convergido", "atr", ref_atr, min(warmup_atr, n))
    ok = sma < 1e-12 and atr < 1e-9
    print("OK: coincide con la referencia" if ok else "DIFERENCIA: revisar")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
