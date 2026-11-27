//! La vela en formación (#558) es **solo visual** (CLAUDE.md §5b: indicadores, patrones, señales y
//! estrategias usan solo velas cerradas). Este test lo verifica en el código del repo:
//! 1. Ningún crate del motor que decide depende de `cfds-ibkr` (solo el gateway, que la muestra, y
//!    la terminal `bolsa`, que muestra velas cerradas).
//! 2. Fuera de `cfds-ibkr` y del módulo `ibkr_live` del gateway (y su arranque en `main.rs`), nadie
//!    nombra los tipos de la vela en formación.
//! 3. No hay conversión de la vela en formación a la vela del motor (`cfds_core::Candle`).

use std::path::{Path, PathBuf};

fn crates_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Los crates que pueden depender de `cfds-ibkr`.
const ALLOWED: [&str; 3] = ["ibkr", "gateway", "terminal"];

/// Nombres de la vela en formación que no pueden aparecer en otro lado.
const LIVE_NAMES: [&str; 7] = [
    "LiveBar",
    "BarUpdate",
    "QuoteState",
    "QuoteEvent",
    "live_feed",
    "cfds_ibkr::live",
    "LiveHub",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if p.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            rust_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn decision_crates_do_not_depend_on_ibkr() {
    let mut checked = 0;
    for e in std::fs::read_dir(crates_dir()).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let manifest = e.path().join("Cargo.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            continue;
        };
        checked += 1;
        if ALLOWED.contains(&name.as_str()) {
            continue;
        }
        assert!(
            !text.contains("cfds-ibkr"),
            "{name} depende de cfds-ibkr: la vela en formación es solo visual"
        );
    }
    // indicators, patterns, strategy, liquidity, regime, risk, backtest, runner, ...
    assert!(checked >= 15, "no se encontraron los crates ({checked})");
    for name in [
        "indicators",
        "patterns",
        "strategy",
        "liquidity",
        "risk",
        "runner",
        "backtest",
    ] {
        assert!(
            crates_dir().join(name).join("Cargo.toml").exists(),
            "falta {name}"
        );
    }
}

#[test]
fn live_bar_names_only_where_it_is_shown() {
    let mut files = Vec::new();
    rust_files(&crates_dir(), &mut files);
    let base = crates_dir();
    for f in files {
        let rel = f
            .strip_prefix(&base)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        let allowed = rel.starts_with("ibkr/")
            || rel == "gateway/src/ibkr_live.rs"
            || rel == "gateway/src/main.rs"
            || rel == "gateway/tests/ibkr_live_route.rs";
        if allowed {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        for n in LIVE_NAMES {
            assert!(
                !text.contains(n),
                "{rel} nombra {n}: la vela en formación es solo visual"
            );
        }
    }
}

#[test]
fn no_conversion_from_live_bar_to_engine_candle() {
    let mut files = Vec::new();
    rust_files(&crates_dir(), &mut files);
    for f in files {
        if f.ends_with("live_isolation.rs") {
            continue;
        }
        let text = std::fs::read_to_string(&f).unwrap();
        for bad in [
            "From<LiveBar>",
            "From<&LiveBar>",
            "Into<Candle> for LiveBar",
        ] {
            assert!(!text.contains(bad), "{}: {bad}", f.display());
        }
    }
    // Y la vela en formación no tiene los campos de una Candle (instrument_id, volume).
    let live = include_str!("../src/live.rs");
    let start = live.find("pub struct LiveBar {").unwrap();
    let body = &live[start..start + live[start..].find('}').unwrap()];
    assert!(!body.contains("instrument_id") && !body.contains("volume"));
}
