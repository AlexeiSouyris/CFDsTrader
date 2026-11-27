//! Laboratorio (#746): el catálogo del motor sale del código, de `config/lab.toml` y de
//! `config/risk.toml`, y falla al arrancar si la config no coincide con el código.

use cfds_backtest::lab::catalog::{ParamKind, Slot};
use cfds_backtest::lab::{LabConfig, StrategyCatalog, catalog};
use cfds_core::Timeframe;
use cfds_risk::RiskConfig;
use rust_decimal::dec;

macro_rules! cfg {
    ($f:literal) => {
        include_str!(concat!("../../../../config/", $f))
    };
}

fn lab_cfg() -> LabConfig {
    LabConfig::from_toml(cfg!("lab.toml")).unwrap()
}

fn risk() -> RiskConfig {
    RiskConfig::from_toml(cfg!("risk.toml")).unwrap()
}

fn engine_catalog() -> StrategyCatalog {
    catalog(&lab_cfg(), &risk(), ["US500".parse().unwrap()]).unwrap()
}

#[test]
fn the_catalog_takes_its_limits_from_lab_and_risk_config() {
    let c = engine_catalog();
    assert_eq!(c.catalog_version, 1);
    assert_eq!(c.limits.max_risk_pct, risk().risk_per_trade_pct);
    assert_eq!(c.limits.min_cost_multiplier, dec!(1));
    assert_eq!(c.limits.max_cost_multiplier, dec!(5));
    assert_eq!(
        (c.limits.max_entry_conditions, c.limits.max_filters),
        (3, 2)
    );
    assert_eq!(c.limits.max_variants_per_run, 20);
    assert_eq!(c.markets.len(), 1);
    assert_eq!(c.markets[0].timeframes, vec![Timeframe::M15, Timeframe::H1]);
    // Solo exploratorio hasta el flujo de pre-registro (L13).
    assert_eq!(c.modes.len(), 1);
}

#[test]
fn the_v1_blocks_are_the_ones_of_the_design() {
    let c = engine_catalog();
    let kinds: Vec<(Slot, &str)> = c.blocks.iter().map(|b| (b.slot, b.kind.as_str())).collect();
    assert_eq!(
        kinds,
        vec![
            (Slot::Entry, "ma_cross"),
            (Slot::Entry, "opening_range_breakout"),
            (Slot::Filter, "entry_hours_utc"),
            (Slot::Stop, "atr"),
            (Slot::Exit, "time_target"),
        ]
    );
    // La ruptura del rango de apertura es solo de 15m.
    let orb = c.block("opening_range_breakout").unwrap();
    assert_eq!(orb.timeframes, vec![Timeframe::M15]);
    // La distancia del stop en ATR es la de config/risk.toml.
    let k = c
        .block("atr")
        .unwrap()
        .params
        .iter()
        .find(|p| p.name == "k")
        .unwrap();
    assert_eq!(
        k.kind,
        ParamKind::Decimal {
            min: risk().min_stop_atr,
            max: risk().max_stop_atr,
            default: Some(dec!(1.5))
        }
    );
    // Las salidas por tiempo y el objetivo son opcionales; el resto, obligatorio.
    let exit = c.block("time_target").unwrap();
    assert!(exit.params.iter().all(|p| !p.required));
    let fast = &c.block("ma_cross").unwrap().params[1];
    assert_eq!(
        (fast.name.as_str(), fast.required, &fast.kind),
        (
            "fast",
            true,
            &ParamKind::Integer {
                min: 2,
                max: 200,
                default: Some(20)
            }
        )
    );
}

#[test]
fn a_config_that_does_not_match_the_code_fails_at_startup() {
    // Falta un parámetro del código.
    let mut missing = lab_cfg();
    missing.params.get_mut("ma_cross").unwrap().remove("slow");
    assert!(catalog(&missing, &risk(), []).unwrap_err().contains("slow"));
    // Un bloque o un parámetro que el código no conoce.
    let unknown = format!(
        "{}\n[params.rsi]\nperiod = {{ min = \"2\", max = \"50\" }}\n",
        cfg!("lab.toml")
    );
    let err = catalog(&LabConfig::from_toml(&unknown).unwrap(), &risk(), []).unwrap_err();
    assert!(err.contains("rsi"), "{err}");
    let extra = cfg!("lab.toml").replace(
        "[params.atr]\n",
        "[params.atr]\nsmoothing = { default = \"1\" }\n",
    );
    let err = catalog(&LabConfig::from_toml(&extra).unwrap(), &risk(), []).unwrap_err();
    assert!(err.contains("smoothing"), "{err}");
    // Rangos o valores propuestos inválidos.
    for (from, to) in [
        ("min = \"2\", max = \"200\"", "min = \"300\", max = \"200\""),
        ("default = \"20\"", "default = \"500\""),
        ("default = \"ema\"", "default = \"wma\""),
        ("default = \"13:30\"", "default = \"25:00\""),
        ("default = \"true\"", "default = \"sí\""),
        ("min = \"0.5\", max = \"10\"", "min = \"-1\", max = \"10\""),
        ("from = { default", "from = { min = \"1\", default"),
        (
            "max_cost_multiplier = \"5\"",
            "max_cost_multiplier = \"0.5\"",
        ),
        ("max_entry_conditions = 3", "max_entry_conditions = 0"),
    ] {
        let text = cfg!("lab.toml").replace(from, to);
        assert_ne!(text, cfg!("lab.toml"), "{from}");
        let parsed = LabConfig::from_toml(&text);
        assert!(
            parsed.is_err() || catalog(&parsed.unwrap(), &risk(), []).is_err(),
            "{to}"
        );
    }
    assert!(LabConfig::from_toml("catalog_version = 1\n").is_err());
}
