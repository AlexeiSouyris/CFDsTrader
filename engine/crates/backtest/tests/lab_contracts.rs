//! Contratos del laboratorio de estrategias (#725, `docs/design/2026-11-laboratorio.md` §6):
//! ejemplos válidos e inválidos de `strategy_catalog.v1`, `strategy_spec.v1` y
//! `spec_validation.v1` contra sus JSON Schema.

use std::path::PathBuf;

use serde_json::{Value, json};

fn validator(name: &str) -> jsonschema::Validator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts")
        .join(name);
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    jsonschema::validator_for(&doc).expect("schema inválido")
}

fn assert_valid(schema: &str, doc: &Value) {
    let errors: Vec<String> = validator(schema)
        .iter_errors(doc)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{schema} rechazó {doc}: {errors:?}");
}

fn assert_invalid(schema: &str, doc: &Value) {
    assert!(
        !validator(schema).is_valid(doc),
        "{schema} debería rechazar {doc}"
    );
}

/// Pisa (o agrega) `pointer` (JSON Pointer) con `value`.
fn with(doc: &Value, pointer: &str, value: Value) -> Value {
    let mut d = doc.clone();
    if let Some(v) = d.pointer_mut(pointer) {
        *v = value;
    } else {
        let (parent, key) = pointer.rsplit_once('/').unwrap();
        d.pointer_mut(parent)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), value);
    }
    d
}

/// Saca la clave final de `pointer`.
fn without(doc: &Value, pointer: &str) -> Value {
    let mut d = doc.clone();
    let (parent, key) = pointer.rsplit_once('/').unwrap();
    d.pointer_mut(parent)
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove(key);
    d
}

const SPEC: &str = "strategy_spec.v1.schema.json";
const CATALOG: &str = "strategy_catalog.v1.schema.json";
const VALIDATION: &str = "spec_validation.v1.schema.json";

fn spec() -> Value {
    json!({
        "catalog_version": 1,
        "market": { "instrument_id": "US500", "source": "futures", "timeframe": "1h" },
        "direction": "both",
        "entry": [
            { "type": "ma_cross", "params": { "average": "ema", "fast": 20, "slow": 50 } }
        ],
        "filters": [
            { "type": "entry_hours_utc", "params": { "from": "13:30", "to": "20:00" } }
        ],
        "stop": { "type": "atr", "params": { "period": 14, "k": "1.5" } },
        "exit": {
            "type": "time_target",
            "params": { "max_candles": 6, "session_close": true, "target_r": "2" }
        },
        "risk_pct": "0.5",
        "cost_multiplier": "1",
        "period": { "from": "2015-01-01T00:00:00Z", "to": "2023-01-01T00:00:00Z" },
        "mode": "exploratory"
    })
}

fn catalog() -> Value {
    json!({
        "catalog_version": 1,
        "limits": {
            "max_entry_conditions": 3,
            "max_filters": 2,
            "max_risk_pct": "1",
            "min_cost_multiplier": "1",
            "max_cost_multiplier": "5",
            "max_variants_per_run": 20
        },
        "markets": [
            { "instrument_id": "US500", "source": "futures", "timeframes": ["15m", "1h"] }
        ],
        "modes": ["exploratory"],
        "blocks": [
            {
                "slot": "entry",
                "type": "ma_cross",
                "label": "Cruce de dos medias",
                "description": "Largo cuando la media rápida cruza arriba de la lenta al cierre.",
                "timeframes": ["15m", "1h"],
                "params": [
                    {
                        "name": "average", "label": "Media", "kind": "enum", "required": true,
                        "options": ["sma", "ema"], "default": "ema"
                    },
                    {
                        "name": "fast", "label": "Rápida", "kind": "integer", "required": true,
                        "min": 2, "max": 200, "default": 20
                    }
                ]
            },
            {
                "slot": "stop",
                "type": "atr",
                "label": "Stop en ATR",
                "description": "k × ATR de Wilder desde el cierre de la señal.",
                "timeframes": ["15m", "1h"],
                "params": [
                    {
                        "name": "k", "label": "k", "kind": "decimal", "required": true,
                        "min": "0.5", "max": "3", "default": "1.5"
                    }
                ]
            },
            {
                "slot": "filter",
                "type": "entry_hours_utc",
                "label": "Horas de entrada (UTC)",
                "description": "Solo entra si la decisión cae en [from, to) UTC.",
                "timeframes": ["15m", "1h"],
                "params": [
                    { "name": "from", "label": "Desde", "kind": "time", "required": true, "default": null }
                ]
            },
            {
                "slot": "exit",
                "type": "time_target",
                "label": "Tiempo y objetivo",
                "description": "Sale a las N velas o al cierre de la sesión; objetivo en R opcional.",
                "timeframes": ["15m", "1h"],
                "params": [
                    {
                        "name": "session_close", "label": "Cierre de sesión", "kind": "boolean",
                        "required": false, "default": true
                    }
                ]
            }
        ]
    })
}

#[test]
fn a_full_spec_is_valid_and_its_fields_are_closed() {
    let s = spec();
    assert_valid(SPEC, &s);
    assert_valid(SPEC, &with(&s, "/filters", json!([])));
    assert_valid(
        SPEC,
        &with(&s, "/preregistration_sha256", json!("a".repeat(64))),
    );
    for field in ["stop", "exit", "entry", "period", "mode", "risk_pct"] {
        assert_invalid(SPEC, &without(&s, &format!("/{field}")));
    }
    assert_invalid(SPEC, &with(&s, "/entry", json!([])));
    assert_invalid(SPEC, &with(&s, "/extra", json!(1)));
    assert_invalid(SPEC, &with(&s, "/stop/extra", json!(1)));
}

#[test]
fn the_spec_rejects_floats_and_unknown_enums() {
    let s = spec();
    // Decimales como texto, nunca como número.
    assert_invalid(SPEC, &with(&s, "/risk_pct", json!(0.5)));
    assert_invalid(SPEC, &with(&s, "/stop/params/k", json!(1.5)));
    assert_invalid(SPEC, &with(&s, "/risk_pct", json!("-1")));
    assert_invalid(SPEC, &with(&s, "/market/source", json!("ig")));
    assert_invalid(SPEC, &with(&s, "/market/timeframe", json!("1d")));
    assert_invalid(SPEC, &with(&s, "/direction", json!("up")));
    assert_invalid(SPEC, &with(&s, "/mode", json!("optimize")));
    assert_invalid(SPEC, &with(&s, "/period/from", json!("2015-01-01")));
    assert_invalid(SPEC, &with(&s, "/entry/0/type", json!("Ma Cross")));
    assert_invalid(SPEC, &with(&s, "/entry/0/params/Fast", json!(20)));
    assert_invalid(SPEC, &with(&s, "/entry/0/params/fast", json!(null)));
}

#[test]
fn the_catalog_describes_every_param_kind() {
    let c = catalog();
    assert_valid(CATALOG, &c);
    // Un decimal con mínimo numérico, un entero con default de texto o un tipo desconocido no.
    assert_invalid(CATALOG, &with(&c, "/blocks/1/params/0/min", json!(0.5)));
    assert_invalid(
        CATALOG,
        &with(&c, "/blocks/0/params/1/default", json!("20")),
    );
    assert_invalid(
        CATALOG,
        &with(&c, "/blocks/0/params/1/kind", json!("float")),
    );
    assert_invalid(CATALOG, &with(&c, "/blocks/0/slot", json!("sizing")));
    assert_invalid(
        CATALOG,
        &with(&c, "/blocks/2/params/0/default", json!("25:00")),
    );
    assert_invalid(CATALOG, &with(&c, "/blocks/0/params/0/options", json!([])));
    assert_invalid(CATALOG, &with(&c, "/modes", json!(["optimize"])));
    assert_invalid(CATALOG, &without(&c, "/limits/max_risk_pct"));
}

#[test]
fn a_validation_is_either_valid_with_hashes_or_invalid_with_errors() {
    let hash = "0123456789abcdef".repeat(4);
    let ok = json!({
        "valid": true,
        "catalog_version": 1,
        "spec_sha256": hash,
        "family_sha256": hash,
        "errors": []
    });
    assert_valid(VALIDATION, &ok);
    let bad = json!({
        "valid": false,
        "catalog_version": 1,
        "spec_sha256": null,
        "family_sha256": null,
        "errors": [
            {
                "code": "param_out_of_range",
                "path": "/entry/0/params/fast",
                "message": "fast tiene que ir de 2 a 200"
            },
            { "code": "period_touches_holdout", "path": "/period/to", "message": "toca el fuera de muestra" },
            { "code": "malformed", "path": "", "message": "falta el campo stop" }
        ]
    });
    assert_valid(VALIDATION, &bad);
    // Válida con errores, inválida sin errores o con hash: no.
    assert_invalid(VALIDATION, &with(&ok, "/errors", bad["errors"].clone()));
    assert_invalid(VALIDATION, &with(&bad, "/errors", json!([])));
    assert_invalid(VALIDATION, &with(&bad, "/spec_sha256", json!(hash)));
    assert_invalid(VALIDATION, &with(&ok, "/spec_sha256", json!(null)));
    assert_invalid(VALIDATION, &with(&bad, "/errors/0/code", json!("oops")));
    assert_invalid(VALIDATION, &with(&bad, "/errors/0/path", json!("entry/0")));
}

// ── Tipos de Rust (#745): lo que Rust lee y escribe dice lo mismo que los schemas ──────────

mod rust_types {
    use super::*;
    use cfds_backtest::lab::{ErrorCode, SpecError, SpecValidation, StrategyCatalog, StrategySpec};
    use serde::Serialize;
    use serde::de::DeserializeOwned;

    fn roundtrip<T>(schema: &str, value: &T)
    where
        T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_value(value).unwrap();
        assert_valid(schema, &json);
        assert_eq!(&serde_json::from_value::<T>(json).unwrap(), value);
    }

    #[test]
    fn a_catalog_roundtrips_with_every_param_kind() {
        let c: StrategyCatalog = serde_json::from_value(catalog()).unwrap();
        roundtrip(CATALOG, &c);
        assert_eq!(serde_json::to_value(&c).unwrap(), catalog());
        assert!(c.block("ma_cross").is_some() && c.block("rsi").is_none());
    }

    #[test]
    fn the_engine_catalog_from_config_matches_the_schema() {
        let c = cfds_backtest::lab::catalog(
            &cfds_backtest::lab::LabConfig::from_toml(include_str!("../../../../config/lab.toml"))
                .unwrap(),
            &cfds_risk::RiskConfig::from_toml(include_str!("../../../../config/risk.toml"))
                .unwrap(),
            ["US500".parse().unwrap(), "US100".parse().unwrap()],
        )
        .unwrap();
        roundtrip(CATALOG, &c);
    }

    #[test]
    fn a_spec_roundtrips_and_rust_rejects_what_the_schema_rejects() {
        let s: StrategySpec = serde_json::from_value(spec()).unwrap();
        roundtrip(SPEC, &s);
        for bad in [
            with(&spec(), "/risk_pct", json!(0.5)),
            with(&spec(), "/risk_pct", json!("-1")),
            with(&spec(), "/extra", json!(1)),
            with(&spec(), "/market/source", json!("ig")),
            with(&spec(), "/market/timeframe", json!("1d")),
            with(&spec(), "/period/from", json!("2015-01-01T00:00:00+01:00")),
            with(&spec(), "/stop/params/k", json!(1.5)),
            with(&spec(), "/stop/extra", json!(1)),
            without(&spec(), "/stop"),
        ] {
            assert_invalid(SPEC, &bad);
            assert!(
                serde_json::from_value::<StrategySpec>(bad.clone()).is_err(),
                "{bad}"
            );
        }
    }

    #[test]
    fn both_validation_shapes_roundtrip() {
        let hash = "0123456789abcdef".repeat(4);
        roundtrip(
            VALIDATION,
            &SpecValidation {
                valid: true,
                catalog_version: 1,
                spec_sha256: Some(hash.clone()),
                family_sha256: Some(hash),
                errors: Vec::new(),
            },
        );
        roundtrip(
            VALIDATION,
            &SpecValidation {
                valid: false,
                catalog_version: 1,
                spec_sha256: None,
                family_sha256: None,
                errors: vec![
                    SpecError::new(ErrorCode::ParamOutOfRange, "/entry/0/params/fast", "fuera"),
                    SpecError::new(ErrorCode::Malformed, "", "falta stop"),
                ],
            },
        );
    }
}
