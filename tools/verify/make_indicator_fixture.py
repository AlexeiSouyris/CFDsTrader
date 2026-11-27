"""Genera el fixture de referencia de indicadores (#666): velas SINTÉTICAS y valores de referencia.

No usa datos de ningún proveedor: las velas salen de un generador congruencial fijo (mismo
archivo en cada corrida), así que el fixture se puede commitear. Calcula dos referencias
independientes del motor:

- `pine_*`: las fórmulas publicadas de TradingView (manual de Pine Script v5, `ta.sma`, `ta.ema`,
  `ta.rma`, `ta.tr(true)` y `ta.atr`), en aritmética **exacta** con `fractions.Fraction` y
  redondeadas a 20 decimales. El motor dice que sigue esta convención: tiene que coincidir
  desde la primera vela.
- `talib_*`: TA-Lib 0.8.3 (C, licencia BSD), en `float`. SMA y EMA usan la misma definición que
  el motor; el ATR de TA-Lib arranca una vela después (ignora el TR de la primera vela), así que
  solo coincide cuando el suavizado de Wilder ya borró la diferencia inicial.

Es una herramienta de verificación, no parte del sistema (ver `tools/verify/README.md`).

    uv run --no-project --with ta-lib==0.8.3 --with numpy \
        python tools/verify/make_indicator_fixture.py \
        engine/crates/indicators/tests/fixtures/reference_indicators.csv
"""

import csv
import sys
from decimal import ROUND_HALF_EVEN, Decimal, localcontext
from fractions import Fraction

import numpy as np
import talib

N_BARS = 300
SMA_N = 10
EMA_N = 10
ATR_N = 14


def synthetic_hlc(n: int) -> list[tuple[Fraction, Fraction, Fraction]]:
    """Paseo al azar reproducible en centésimos, con gaps, siempre positivo."""
    state = 20261125

    def rand(k: int) -> int:
        nonlocal state
        state = (1103515245 * state + 12345) % 2**31
        return state % k

    out = []
    close = 1_000_000  # 10 000,00
    for _ in range(n):
        gap = rand(3001) - 1500 if rand(5) == 0 else 0
        open_ = close + gap
        close = open_ + rand(4001) - 2000
        high = max(open_, close) + rand(1501)
        low = min(open_, close) - rand(1501)
        out.append((Fraction(high, 100), Fraction(low, 100), Fraction(close, 100)))
    return out


def pine_sma(x: list[Fraction], n: int) -> list[Fraction | None]:
    return [sum(x[i + 1 - n : i + 1]) / n if i + 1 >= n else None for i in range(len(x))]


def pine_smoothed(x: list[Fraction], n: int, alpha: Fraction) -> list[Fraction | None]:
    """`ta.ema` (alpha = 2/(n+1)) y `ta.rma` (alpha = 1/n): sembradas con la SMA de n valores."""
    seed = pine_sma(x, n)
    out: list[Fraction | None] = []
    prev: Fraction | None = None
    for i, v in enumerate(x):
        prev = seed[i] if prev is None else alpha * v + (1 - alpha) * prev
        out.append(prev)
    return out


def pine_tr(hlc: list[tuple[Fraction, Fraction, Fraction]]) -> list[Fraction]:
    """`ta.tr(true)`: en la primera vela, sin cierre previo, H - L."""
    out = []
    for i, (h, low, _) in enumerate(hlc):
        if i == 0:
            out.append(h - low)
        else:
            pc = hlc[i - 1][2]
            out.append(max(h - low, abs(h - pc), abs(low - pc)))
    return out


def fmt_exact(v: Fraction | None) -> str:
    if v is None:
        return ""
    with localcontext() as ctx:
        ctx.prec = 60
        d = Decimal(v.numerator) / Decimal(v.denominator)
        return str(d.quantize(Decimal("1e-20"), rounding=ROUND_HALF_EVEN))


def fmt_price(v: Fraction) -> str:
    return str((Decimal(v.numerator) / Decimal(v.denominator)).quantize(Decimal("0.01")))


def fmt_float(v: float) -> str:
    return "" if np.isnan(v) else repr(float(v))


def main(path: str) -> int:
    hlc = synthetic_hlc(N_BARS)
    closes = [c for _, _, c in hlc]
    pine = {
        "pine_sma10": pine_sma(closes, SMA_N),
        "pine_ema10": pine_smoothed(closes, EMA_N, Fraction(2, EMA_N + 1)),
        "pine_atr14": pine_smoothed(pine_tr(hlc), ATR_N, Fraction(1, ATR_N)),
    }
    h = np.array([float(x) for x, _, _ in hlc])
    lo = np.array([float(x) for _, x, _ in hlc])
    c = np.array([float(x) for x in closes])
    talib_cols = {
        "talib_sma10": talib.SMA(c, timeperiod=SMA_N),
        "talib_ema10": talib.EMA(c, timeperiod=EMA_N),
        "talib_atr14": talib.ATR(h, lo, c, timeperiod=ATR_N),
    }
    with open(path, "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["i", "high", "low", "close", *pine, *talib_cols])
        for i, (hi, low, cl) in enumerate(hlc):
            w.writerow(
                [i, fmt_price(hi), fmt_price(low), fmt_price(cl)]
                + [fmt_exact(col[i]) for col in pine.values()]
                + [fmt_float(col[i]) for col in talib_cols.values()]
            )
    print(f"{path}: {N_BARS} velas sintéticas")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
