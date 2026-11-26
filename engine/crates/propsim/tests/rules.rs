//! Reglas (`prop_rules.v1`) y plantillas de `config/propsim.toml` (#802).

mod common;

use cfds_propsim::{PropsimConfig, RulesDoc};
use common::{assert_valid, config, rules_json, with};
use rust_decimal::dec;
use serde_json::json;

const RULES: &str = "prop_rules.v1.schema.json";

#[test]
fn the_repo_config_has_only_the_blank_generic_template() {
    let cfg = config();
    assert_eq!(cfg.templates.len(), 1);
    let t = &cfg.templates[0];
    assert_eq!(t.id, "generica");
    assert!(t.source.is_none());
    assert!(!t.has_values(), "la genérica va en blanco");
    let doc = serde_json::to_value(t).unwrap();
    assert_valid(RULES, &doc);
    assert_eq!(doc["account_size"], json!(null));
    assert_eq!(doc["phases"][0]["profit_target_pct"], json!(null));
    assert_eq!(doc["daily_loss"]["limit_pct"], json!(null));
}

#[test]
fn the_blank_template_lists_every_missing_value() {
    let t = &config().templates[0];
    let errors = t.validate().unwrap_err();
    let paths: Vec<&str> = errors.iter().map(|e| e.path.as_str()).collect();
    for p in [
        "/account_size",
        "/currency",
        "/day",
        "/phases/0/profit_target_pct",
        "/daily_loss/limit_pct",
        "/max_drawdown/limit_pct",
        "/funded/profit_split_pct",
        "/fees/evaluation",
    ] {
        assert!(paths.contains(&p), "falta {p} en {paths:?}");
    }
}

#[test]
fn filled_rules_validate_into_money_and_fractions() {
    let doc = rules_json();
    assert_valid(RULES, &doc);
    let r: RulesDoc = serde_json::from_value(doc).unwrap();
    let r = r.validate().unwrap();
    assert_eq!(r.phases[0].target, dec!(10000));
    assert_eq!(r.daily_loss.unwrap().limit, dec!(0.05));
    assert_eq!(r.max_drawdown.unwrap().amount, dec!(10000));
    assert_eq!(r.fees.activation, dec!(0), "null = no hay");
    assert_eq!(r.phases[0].min_days, 0);
}

#[test]
fn bad_values_are_reported_with_their_path() {
    for (ptr, v, path) in [
        ("/account_size", json!("0"), "/account_size"),
        ("/currency", json!("usd"), "/currency"),
        ("/day/timezone", json!("Marte/Base"), "/day/timezone"),
        ("/day/cutoff", json!("25:00"), "/day/cutoff"),
        (
            "/phases/0/profit_target_pct",
            json!("120"),
            "/phases/0/profit_target_pct",
        ),
        (
            "/phases/0/max_trading_days",
            json!(0),
            "/phases/0/max_trading_days",
        ),
        ("/daily_loss/limit_pct", json!("0"), "/daily_loss/limit_pct"),
        ("/phases", json!([]), "/phases"),
        (
            "/fees/refund_with_first_payout",
            json!(true),
            "/fees/refund_with_first_payout",
        ),
    ] {
        let d: RulesDoc = serde_json::from_value(with(rules_json(), ptr, v)).unwrap();
        let errs = d.validate().unwrap_err();
        assert!(errs.iter().any(|e| e.path == path), "{ptr}: {errs:?}");
    }
    let min_over_max = with(rules_json(), "/phases/0/min_trading_days", json!(5));
    let min_over_max = with(min_over_max, "/phases/0/max_trading_days", json!(3));
    let d: RulesDoc = serde_json::from_value(min_over_max).unwrap();
    assert!(d.validate().is_err());
    // Campos que el contrato no tiene: ni se leen.
    let mut extra = rules_json();
    extra["leverage"] = json!("100");
    assert!(serde_json::from_value::<RulesDoc>(extra).is_err());
}

#[test]
fn a_template_with_values_needs_the_published_rulebook() {
    let base = include_str!("../../../../config/propsim.toml");
    let filled = base.replace(
        "phases = [{ name = \"Evaluación\" }]",
        "account_size = \"100000\"\nphases = [{ name = \"Evaluación\" }]",
    );
    assert_ne!(filled, base);
    let err = PropsimConfig::from_toml(&filled).unwrap_err();
    assert!(err.contains("source"), "{err}");
    // Con fuente pero sin enlace https: tampoco.
    let bad_source = filled.replace(
        "name = \"Plantilla genérica (completá los valores)\"",
        "name = \"X\"\nsource = { firm = \"X\", url = \"http://x\", read_at = \"2026-11-26\" }",
    );
    assert!(PropsimConfig::from_toml(&bad_source).is_err());
    // Plantillas repetidas o sin la genérica: no arranca.
    let twice = format!("{base}\n{}", &base[base.find("[[templates]]").unwrap()..]);
    assert!(
        PropsimConfig::from_toml(&twice)
            .unwrap_err()
            .contains("repetida")
    );
    let none = &base[..base.find("[[templates]]").unwrap()];
    assert!(PropsimConfig::from_toml(none).is_err());
}
