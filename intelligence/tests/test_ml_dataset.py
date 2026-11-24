"""Dataset del modelo de ML v1 (docs/spec/ml-v1.md §1 y §3, #158), con filas sintéticas."""

import json
from dataclasses import replace
from datetime import UTC, datetime, timedelta
from decimal import Decimal
from pathlib import Path

import jsonschema
import pytest

from cfds_intel.models.config import MlConfigError, load_config, load_default
from cfds_intel.models.dataset import DatasetError, Row, build, encode, feature_names

ROOT = Path(__file__).resolve().parents[2]
CFG = load_default().dataset
T0 = datetime(2026, 11, 20, 15, 0, tzinfo=UTC)


def row(k: int = 0, **kw: object) -> Row:
    at = T0 + timedelta(minutes=15 * k)
    base = Row(
        proposal_id=f"bt:US500:{k}:long",
        instrument_id="US500",
        side="long",
        signal_at=at,
        reference=Decimal("100"),
        stop=Decimal("98"),
        tp1=Decimal("103"),
        score=6,
        breakdown=[{"component": "trend", "points": 2}, {"component": "sweep", "points": 2}],
        triggers=["sweep"],
        hit_tp1=True,
        exits=[
            {"at": (at + timedelta(hours=1)).isoformat().replace("+00:00", "Z")},
            {"at": (at + timedelta(hours=3)).isoformat().replace("+00:00", "Z")},
        ],
        as_of=at,
        atr14_m15=Decimal("4"),
        realized_vol_tercile="mid",
        session_band="open",
    )
    return replace(base, **kw)  # type: ignore[arg-type]


def test_the_repository_config_is_us500_only() -> None:
    assert CFG.instruments == ("US500",)
    for bad in [
        "",
        "[dataset]\ninstruments = []",
        '[dataset]\ninstruments = ["us500"]',
        '[dataset]\ninstruments = ["US500", "US500"]',
    ]:
        with pytest.raises(MlConfigError):
            load_config(bad)


def test_one_proposal_becomes_its_features_and_label() -> None:
    e = encode(row(), CFG)
    x = dict(zip(feature_names(CFG), e.x, strict=True))
    assert x["side_long"] == 1.0
    assert x["instrument_US500"] == 1.0
    assert x["trigger_sweep"] == 1.0 and x["trigger_double_break"] == 0.0
    assert x["points_trend"] == 2.0 and x["points_news"] == 0.0
    assert x["score"] == 6.0
    assert x["stop_atr"] == 0.5  # |100 - 98| / 4
    assert x["tp1_r"] == 1.5  # |103 - 100| / 2
    assert (x["vol_low"], x["vol_mid"], x["vol_high"]) == (0.0, 1.0, 0.0)
    assert (x["session_open"], x["session_mid"], x["session_close"]) == (1.0, 0.0, 0.0)
    assert e.y == 1
    assert e.known_at == T0 + timedelta(hours=3), "el resultado se conoce en la última salida"


def test_shorts_use_distances_not_signs() -> None:
    e = encode(
        row(side="short", reference=Decimal("100"), stop=Decimal("102"), tp1=Decimal("97")), CFG
    )
    x = dict(zip(feature_names(CFG), e.x, strict=True))
    assert (x["side_long"], x["stop_atr"], x["tp1_r"]) == (0.0, 0.5, 1.5)


def test_time_exits_and_forced_closes_are_zero() -> None:
    # Decisión 1 de la spec §11: no tocó TP1 → 0, sea cual sea el motivo de la salida.
    assert encode(row(hit_tp1=False), CFG).y == 0


def test_what_does_not_enter_is_counted_apart_never_filled() -> None:
    rows = [
        row(3),
        row(1),
        row(2, instrument_id="US100"),
        row(4, hit_tp1=None, exits=None),
        row(5, as_of=None, atr14_m15=None, realized_vol_tercile=None, session_band=None),
    ]
    d = build(rows, CFG)
    assert [e.proposal_id for e in d.examples] == ["bt:US500:1:long", "bt:US500:3:long"]
    assert (d.other_instrument, d.without_outcome, d.without_features) == (1, 1, 1)


def test_features_from_another_time_are_an_error() -> None:
    # Solo información al cierre de t (§3): as_of tiene que ser signal_at.
    with pytest.raises(DatasetError):
        encode(row(as_of=T0 + timedelta(minutes=15)), CFG)
    with pytest.raises(DatasetError):
        encode(row(atr14_m15=Decimal("0")), CFG)


def test_an_example_depends_only_on_its_own_row() -> None:
    # Sin fuga entre filas: agregar propuestas posteriores no cambia las anteriores (§8.2).
    early = build([row(1), row(2)], CFG).examples
    later = build([row(1), row(2), row(3, hit_tp1=False)], CFG).examples
    assert later[: len(early)] == early


def test_ml_features_contract_matches_what_the_dataset_reads() -> None:
    schema = json.loads((ROOT / "contracts" / "ml_features.v1.schema.json").read_text())
    ok = {
        "proposal_id": "bt:US500:2026-11-20T15:00:00Z:long",
        "as_of": "2026-11-20T15:00:00Z",
        "atr14_m15": "4.25",
        "realized_vol_tercile": "mid",
        "session_band": "close",
    }
    jsonschema.validate(ok, schema)
    for key, bad in [
        ("atr14_m15", 4.25),
        ("realized_vol_tercile", "medium"),
        ("session_band", "lunch"),
        ("as_of", "2026-11-20T15:00:00+01:00"),
    ]:
        with pytest.raises(jsonschema.ValidationError):
            jsonschema.validate({**ok, key: bad}, schema)
    assert set(schema["properties"]["realized_vol_tercile"]["enum"]) == {"low", "mid", "high"}
    assert set(schema["properties"]["session_band"]["enum"]) == {"open", "mid", "close"}


def test_the_futures_source_reads_only_derived_ids_and_never_mixes() -> None:
    # #400: las propuestas del backtest con futuros llevan `US500.FUT`; se codifican como US500.
    rows = [row(1, instrument_id="US500.FUT"), row(2), row(3, instrument_id="US100.FUT")]
    fut = build(rows, CFG, ".FUT")
    assert [e.proposal_id for e in fut.examples] == ["bt:US500:1:long"]
    assert fut.other_instrument == 2, "el CFD y otro índice quedan afuera"
    x = dict(zip(feature_names(CFG), fut.examples[0].x, strict=True))
    assert x["instrument_US500"] == 1.0
    cfd = build(rows, CFG)
    assert [e.proposal_id for e in cfd.examples] == ["bt:US500:2:long"]
    assert cfd.other_instrument == 2, "sin sufijo, los futuros nunca entran"
    assert fut.examples[0].x == cfd.examples[0].x, "misma codificación en las dos fuentes"
