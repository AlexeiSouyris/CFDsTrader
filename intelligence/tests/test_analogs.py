"""Análogos v1 (#148): forma, sin look-ahead, sin repetir episodios, resultados y contexto.

Sin base ni red: series sintéticas de velas diarias.
"""

from dataclasses import replace
from datetime import UTC, date, datetime, timedelta
from decimal import Decimal
from itertools import pairwise
from pathlib import Path

import pytest

from cfds_intel.analogs.config import AnalogsConfigError, load_config
from cfds_intel.analogs.data import benchmarks, daily_params, futures_suffix, select_source
from cfds_intel.analogs.search import (
    Bar,
    Rate,
    analyze,
    dtw,
    find_neighbors,
    hit_first,
    nearest_rank,
    rate_at,
    rates_trend,
    zscore,
)

ROOT = Path(__file__).resolve().parents[2]
CFG = load_config((ROOT / "config" / "analogs.toml").read_text())
# Chica para los tests: ventana de 3, horizonte máximo de 2, 2 vecinos.
SMALL = replace(
    CFG, window=3, dtw_band=1, neighbors=2, min_neighbors=2, horizons=(1, 2), r_horizon=2
)


def bars(closes: list[str], first: date = date(2024, 1, 1), atr: str | None = "1") -> list[Bar]:
    out = []
    for i, c in enumerate(closes):
        d = first + timedelta(days=i)
        close = Decimal(c)
        out.append(
            Bar(
                day=d,
                end=datetime(d.year, d.month, d.day, 23, tzinfo=UTC),
                high=close + Decimal("0.5"),
                low=close - Decimal("0.5"),
                close=close,
                atr=Decimal(atr) if atr else None,
                vol_tercile=None,
            )
        )
    return out


# ---------- configuración ----------


def test_real_configs_load() -> None:
    assert (CFG.window, CFG.dtw_band, CFG.neighbors, CFG.horizons) == (20, 2, 20, (1, 5, 20))
    assert CFG.max_horizon == 20
    indicators = (ROOT / "config" / "indicators.toml").read_text()
    assert daily_params(indicators) == (14, 20, 3)
    assert benchmarks((ROOT / "config" / "instruments.toml").read_text()) == {
        "DE40": "ESTR",
        "US100": "SOFR",
        "US30": "SOFR",
        "US500": "SOFR",
    }


@pytest.mark.parametrize(
    ("old", "new"),
    [
        ("window = 20", "window = 1"),
        ("min_neighbors = 20", "min_neighbors = 21"),
        ("horizons = [1, 5, 20]", "horizons = []"),
        ("r_up = 2", "r_up = 0"),
        ('rates_threshold_pp = "0.10"', 'rates_threshold_pp = "x"'),
    ],
)
def test_bad_config_is_rejected(old: str, new: str) -> None:
    text = (ROOT / "config" / "analogs.toml").read_text()
    assert old in text
    with pytest.raises(AnalogsConfigError):
        load_config(text.replace(old, new))


# ---------- forma ----------


def test_zscore_ignores_scale_and_rejects_flat_windows() -> None:
    a = zscore([Decimal(x) for x in ("1", "2", "3")])
    b = zscore([Decimal(x) for x in ("100", "200", "300")])
    assert a is not None and b is not None
    assert a == pytest.approx(b)
    assert zscore([Decimal(5)] * 4) is None


def test_dtw_is_zero_for_identical_and_symmetric() -> None:
    a, b = [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]
    assert dtw(a, a, 1) == 0
    assert dtw(a, b, 1) == dtw(b, a, 1)
    assert dtw(a, b, 0) >= dtw(a, b, 1)  # sin banda no puede alinear mejor


def test_the_same_shape_elsewhere_is_the_nearest_neighbor() -> None:
    # Consulta: 10, 12, 11 al final. Hay una copia escalada (100, 120, 110) en otro índice.
    query = bars(["50", "51", "53", "52", "54", "10", "12", "11"])
    other = bars(["100", "120", "110", "105", "107", "108", "120", "121"])
    nb = find_neighbors({"Q": query, "O": other}, "Q", 7, SMALL)
    assert nb[0].instrument == "O"
    assert nb[0].end_index == 2
    assert nb[0].distance == pytest.approx(0)


def test_no_look_ahead_the_whole_outcome_must_exist_before_the_query() -> None:
    query = bars(["1", "3", "2", "4", "3", "5"])
    # En la otra serie, la ventana que termina el día 2 necesita los días 3 y 4 cerrados.
    other = bars(["1", "3", "2", "9", "9", "9"], first=date(2024, 1, 3))
    nb = find_neighbors({"Q": query, "O": other}, "Q", 5, SMALL)
    # El día 4 de "O" es el 7-1: cierra después de la consulta (6-1). No puede entrar.
    assert all(n.instrument != "O" for n in nb)


def test_neighbors_never_overlap_the_query_or_each_other() -> None:
    series = {"Q": bars([str(x) for x in (1, 3, 2, 4, 3, 5, 4, 6, 5, 7, 6, 8, 7, 9)])}
    q_end = len(series["Q"]) - 1
    nb = find_neighbors(series, "Q", q_end, replace(SMALL, neighbors=10, min_neighbors=1))
    ends = sorted(n.end_index for n in nb)
    assert all(q_end - e >= SMALL.window for e in ends)
    assert all(b - a >= SMALL.window for a, b in pairwise(ends))


# ---------- qué pasó después ----------


def test_hit_first_up_down_both_and_none() -> None:
    cfg = replace(SMALL, r_up=Decimal(2), r_down=Decimal(1), r_horizon=3)

    def series(highs: list[str], lows: list[str]) -> list[Bar]:
        b = bars(["100"] * (len(highs) + 1), atr="10")
        return [b[0]] + [
            replace(x, high=Decimal(h), low=Decimal(lo))
            for x, h, lo in zip(b[1:], highs, lows, strict=True)
        ]

    assert hit_first(series(["121", "100", "100"], ["95", "95", "95"]), 0, cfg) == "up"
    assert hit_first(series(["105", "100", "100"], ["90", "95", "95"]), 0, cfg) == "down"
    # Una misma vela toca +2R y -1R: cuenta como -1R (conservador).
    assert hit_first(series(["125", "100", "100"], ["85", "95", "95"]), 0, cfg) == "down"
    assert hit_first(series(["105", "105", "105"], ["95", "95", "95"]), 0, cfg) == "none"
    assert hit_first(bars(["100", "200", "50", "1"], atr=None), 0, cfg) is None


def test_nearest_rank_percentiles() -> None:
    vals = [Decimal(x) for x in range(1, 11)]
    assert nearest_rank(vals, 10) == 1
    assert nearest_rank(vals, 50) == 5
    assert nearest_rank(vals, 90) == 9
    assert nearest_rank([Decimal(7)], 25) == 7


# ---------- contexto ----------


def test_the_vol_context_is_the_tercile_from_rust() -> None:
    # El contexto toma el tercio que guardó Rust (`daily_vol_terciles`) tal cual: no lo calcula.
    b = bars([str(100 + i % 7) for i in range(40)])
    b = [replace(x, vol_tercile="mid") for x in b]
    b[-1] = replace(b[-1], vol_tercile="high")
    r = analyze({"US500": b}, {}, "US500", len(b) - 1, SMALL)
    assert r.context.vol == "high"
    b[-1] = replace(b[-1], vol_tercile=None)
    assert analyze({"US500": b}, {}, "US500", len(b) - 1, SMALL).context.vol is None


def test_rate_in_force_uses_the_newest_published_day() -> None:
    t = datetime(2026, 11, 13, 8, tzinfo=UTC)
    rates = [
        Rate(date(2026, 11, 12), t, Decimal("2.443")),
        Rate(date(2026, 11, 13), t + timedelta(days=3), Decimal("2.437")),
        # Corrección tardía del 12: no pisa al 13, que es más nuevo.
        Rate(date(2026, 11, 12), t + timedelta(days=4), Decimal("9.9")),
    ]
    assert rate_at(rates, t - timedelta(seconds=1)) is None
    assert rate_at(rates, t) == Decimal("2.443")
    assert rate_at(rates, t + timedelta(days=5)) == Decimal("2.437")


def test_rates_trend_up_down_stable_and_missing() -> None:
    cfg = replace(SMALL, rates_lookback=2, rates_threshold_pp=Decimal("0.10"))
    b = bars(["1"] * 3)
    start = b[0].end - timedelta(hours=1)

    def rates(before: str, now: str) -> list[Rate]:
        return [
            Rate(date(2023, 12, 30), start, Decimal(before)),
            Rate(date(2023, 12, 31), b[2].end - timedelta(hours=1), Decimal(now)),
        ]

    assert rates_trend(b, 2, rates("2.0", "2.2"), cfg) == "sube"
    assert rates_trend(b, 2, rates("2.0", "1.8"), cfg) == "baja"
    assert rates_trend(b, 2, rates("2.0", "2.05"), cfg) == "estable"
    assert rates_trend(b, 2, None, cfg) is None
    assert rates_trend(b, 1, rates("2.0", "2.2"), cfg) is None  # no hay 2 sesiones atrás


# ---------- informe ----------


def test_analyze_reports_insufficient_data_and_empty_context_without_rates() -> None:
    series = {"Q": bars([str(x) for x in (1, 3, 2, 4, 3, 5, 4, 6, 5, 7, 6, 8)])}
    r = analyze(series, {}, "Q", 11, replace(SMALL, min_neighbors=2))
    assert r.day == series["Q"][11].day
    assert r.all_neighbors.count == len(r.neighbors) <= SMALL.neighbors
    assert r.all_neighbors.insufficient == (len(r.neighbors) < 2)
    # Sin volatilidad ni tasas, la consulta no tiene contexto: el segundo grupo queda vacío.
    assert r.context.vol is None and r.context.rates is None
    assert r.same_context.count == 0


def test_sources_are_never_mixed_and_futures_use_the_base_rates() -> None:
    """#282: las diarias de IG y las de los futuros son el mismo mercado: nunca juntas."""
    suffix = futures_suffix((ROOT / "config" / "candle_sources.toml").read_text())
    s = {"US500": bars(["1", "2"]), "US500" + suffix: bars(["3", "4"]), "DE40": bars(["5"])}
    r = {"US500": [Rate(date(2024, 1, 1), datetime(2024, 1, 2, tzinfo=UTC), Decimal("5"))]}
    ig, ig_rates = select_source(s, r, None)
    assert sorted(ig) == ["DE40", "US500"]
    assert ig_rates == r
    fut, fut_rates = select_source(s, r, suffix)
    assert list(fut) == ["US500" + suffix]
    assert fut_rates == {"US500" + suffix: r["US500"]}
