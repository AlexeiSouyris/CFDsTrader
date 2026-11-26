//! Recorrido determinista de `cfds-propsim` (#802): cada regla con sus bordes.

mod common;

use cfds_propsim::{DayRef, End, FailRule, Rules, group_days, walk};
use chrono::{DateTime, Utc};
use common::{rules, rules_json, with};
use rust_decimal::Decimal;
use rust_decimal::dec;
use serde_json::json;

/// Corre días de P/L (sin fecha) por las reglas.
fn run(r: &Rules, days: &[&[Decimal]]) -> cfds_propsim::Walk {
    let mut it = days.iter().map(|d| DayRef { date: None, pnl: d });
    walk(r, &mut it)
}

fn one(r: &Rules, days: &[&[Decimal]]) -> cfds_propsim::StageResult {
    run(r, days).phases[0]
}

#[test]
fn reaching_the_target_passes_at_the_end_of_the_day() {
    let r = rules(&rules_json());
    let s = one(&r, &[&[dec!(4000)], &[dec!(3000), dec!(3000)], &[dec!(1)]]);
    assert_eq!(s.end, End::Passed);
    assert_eq!(s.trading_days, 2);
    assert_eq!(s.profit, dec!(10000));
    assert_eq!(s.best_day, Some(dec!(6000)));
    // El objetivo tocado a mitad del día no alcanza si el día cierra por debajo.
    let s = one(&r, &[&[dec!(10000), dec!(-1)], &[dec!(1)]]);
    assert_eq!(s.end, End::Passed);
    assert_eq!(s.trading_days, 2);
}

#[test]
fn daily_loss_exactly_at_the_limit_fails_and_just_below_does_not() {
    let r = rules(&rules_json());
    let s = one(&r, &[&[dec!(-2000), dec!(-3000)]]);
    assert_eq!(s.end, End::Failed(FailRule::DailyLoss));
    assert_eq!(s.trading_days, 1);
    let s = one(&r, &[&[dec!(-2000), dec!(-2999.99)], &[dec!(-1)]]);
    assert_eq!(s.end, End::Unfinished);
    assert_eq!(s.min_balance, dec!(94999.01));
    // La pérdida se mide desde el saldo al inicio de cada día, no desde el máximo del día.
    let s = one(&r, &[&[dec!(3000), dec!(-7000)]]);
    assert_eq!(s.end, End::Unfinished);
}

#[test]
fn daily_loss_on_the_day_start_balance_shrinks_after_losing_days() {
    let doc = with(rules_json(), "/daily_loss/base", json!("day_start"));
    let doc = with(doc, "/max_drawdown", json!(null));
    let r = rules(&doc);
    // Día 1: −4000 → saldo 96 000; día 2: límite 5 % de 96 000 = 4800.
    let s = one(&r, &[&[dec!(-4000)], &[dec!(-4800)]]);
    assert_eq!(s.end, End::Failed(FailRule::DailyLoss));
    let r = rules(&rules_json()); // con base = tamaño, 4800 < 5000 no falla
    let s = one(&r, &[&[dec!(-4000)], &[dec!(-4800)]]);
    assert_eq!(s.end, End::Unfinished);
}

#[test]
fn static_drawdown_fails_when_the_balance_touches_the_floor() {
    let r = rules(&rules_json());
    let days: [&[Decimal]; 3] = [&[dec!(-4000)], &[dec!(-4000)], &[dec!(-2000)]];
    let s = one(&r, &days);
    assert_eq!(s.end, End::Failed(FailRule::MaxDrawdown));
    assert_eq!(s.trading_days, 3);
    assert_eq!(s.profit, dec!(-10000));
    let days: [&[Decimal]; 3] = [&[dec!(-4000)], &[dec!(-4000)], &[dec!(-1999)]];
    assert_eq!(one(&r, &days).end, End::Unfinished);
}

#[test]
fn trailing_drawdown_follows_the_end_of_day_high_and_can_lock() {
    let doc = with(rules_json(), "/max_drawdown/mode", json!("trailing_eod"));
    let doc = with(doc, "/daily_loss", json!(null));
    let doc = with(doc, "/phases/0/profit_target_pct", json!("50"));
    let r = rules(&doc);
    // +8000 al cierre: piso 98 000. Bajar a 98 000 falla aunque esté sobre el piso fijo.
    let s = one(&r, &[&[dec!(8000)], &[dec!(-10000)]]);
    assert_eq!(s.end, End::Failed(FailRule::MaxDrawdown));
    // El máximo dentro del día no mueve el piso: +9000 y −9000 el mismo día cierran en 100 000.
    let s = one(&r, &[&[dec!(9000), dec!(-9000)], &[dec!(-9999)]]);
    assert_eq!(s.end, End::Unfinished);
    // Trabado en el saldo inicial: con +15 000 el piso queda en 100 000, no en 105 000.
    let locked = rules(&with(
        doc.clone(),
        "/max_drawdown/lock_at_initial",
        json!(true),
    ));
    let s = one(&locked, &[&[dec!(15000)], &[dec!(-9999)]]);
    assert_eq!(s.end, End::Unfinished);
    let s = one(&locked, &[&[dec!(15000)], &[dec!(-15000)]]);
    assert_eq!(s.end, End::Failed(FailRule::MaxDrawdown));
    let s = one(&r, &[&[dec!(15000)], &[dec!(-10000)]]);
    assert_eq!(
        s.end,
        End::Failed(FailRule::MaxDrawdown),
        "sin traba: piso 105 000"
    );
}

#[test]
fn intraday_trailing_is_not_simulable_and_is_left_out() {
    let doc = with(
        rules_json(),
        "/max_drawdown/mode",
        json!("trailing_intraday"),
    );
    let r = rules(&doc);
    assert_eq!(r.unsimulable(), vec!["max_drawdown.trailing_intraday"]);
    // Sin la regla de caída: 3 días de −4000 (debajo del 5 % diario) no fallan.
    let days: [&[Decimal]; 3] = [&[dec!(-4000)], &[dec!(-4000)], &[dec!(-4000)]];
    assert_eq!(one(&r, &days).end, End::Unfinished);
    assert!(rules(&rules_json()).unsimulable().is_empty());
}

#[test]
fn minimum_days_delay_the_pass_and_maximum_days_end_the_time() {
    let doc = with(rules_json(), "/phases/0/min_trading_days", json!(3));
    let r = rules(&doc);
    let s = one(&r, &[&[dec!(11000)], &[dec!(10)], &[dec!(-10)]]);
    assert_eq!(s.end, End::Passed);
    assert_eq!(s.trading_days, 3);
    // Si en el día mínimo ya no está sobre el objetivo, sigue.
    let s = one(
        &r,
        &[&[dec!(11000)], &[dec!(10)], &[dec!(-2000)], &[dec!(1000)]],
    );
    assert_eq!(s.trading_days, 4);
    assert_eq!(s.end, End::Passed);
    let doc = with(rules_json(), "/phases/0/max_trading_days", json!(2));
    let r = rules(&doc);
    let s = one(&r, &[&[dec!(1000)], &[dec!(1000)], &[dec!(9000)]]);
    assert_eq!(s.end, End::Timeout);
    assert_eq!(s.trading_days, 2);
}

#[test]
fn consistency_needs_more_days_when_one_day_made_most_of_the_profit() {
    let doc = with(rules_json(), "/phases/0/consistency_pct", json!("50"));
    let r = rules(&doc);
    // 10 000 en un día = 100 % de la ganancia: sigue. Al sumar 10 000 más en dos días, el mejor
    // día es el 50 % de 20 000: pasa.
    let s = one(&r, &[&[dec!(10000)], &[dec!(5000)], &[dec!(5000)]]);
    assert_eq!(s.end, End::Passed);
    assert_eq!(s.trading_days, 3);
    assert_eq!(s.profit, dec!(20000));
}

#[test]
fn phases_start_over_and_the_funded_stage_pays_the_split() {
    let doc = with(
        rules_json(),
        "/phases",
        json!([
            { "name": "Fase 1", "profit_target_pct": "8", "min_trading_days": null,
              "max_trading_days": null, "consistency_pct": null },
            { "name": "Fase 2", "profit_target_pct": "5", "min_trading_days": null,
              "max_trading_days": null, "consistency_pct": null }
        ]),
    );
    let doc = with(
        doc,
        "/funded",
        json!({ "payout_min_trading_days": 2, "payout_min_profit_pct": "1",
            "profit_split_pct": "80", "consistency_pct": null }),
    );
    let r = rules(&doc);
    let w = run(
        &r,
        &[
            &[dec!(8000)],
            &[dec!(5000)],
            &[dec!(2000)],
            &[dec!(-500)],
            &[dec!(100)],
        ],
    );
    assert!(w.passed_evaluation(&r));
    assert_eq!(w.phases[1].profit, dec!(5000), "la fase 2 arranca de nuevo");
    let f = w.funded.unwrap();
    assert_eq!(f.end, End::Passed);
    assert_eq!(f.trading_days, 2);
    assert_eq!(f.payout, Some(dec!(1200)));
    assert!(w.paid());
    // Perder en la fase 1 corta el recorrido.
    let w = run(&r, &[&[dec!(-5000)], &[dec!(9000)]]);
    assert_eq!(w.phases.len(), 1);
    assert!(w.funded.is_none() && !w.paid());
}

#[test]
fn trades_are_grouped_by_the_firm_day_in_its_time_zone() {
    let doc = with(
        rules_json(),
        "/day",
        json!({ "timezone": "America/New_York", "cutoff": "17:00" }),
    );
    let r = rules(&doc);
    let at = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
    // 26-11-2026, Nueva York en UTC−5: 16:59 local = 21:59Z (día que empezó el 25 a las 17:00);
    // 17:00 local = 22:00Z (día nuevo). Desordenadas a propósito.
    let trades = [
        (at("2026-11-26T22:00:00Z"), dec!(-3000)),
        (at("2026-11-26T21:59:00Z"), dec!(-3000)),
        (at("2026-11-26T14:00:00Z"), dec!(-1000)),
    ];
    let days = group_days(&r, &trades);
    assert_eq!(days.len(), 2);
    assert_eq!(days[0].1, vec![dec!(-1000), dec!(-3000)]);
    assert_eq!(days[1].1, vec![dec!(-3000)]);
    assert_eq!(days[0].0.to_string(), "2026-11-25");
    // En UTC con corte a medianoche las tres caen el mismo día y pierden 7000 > 5000: falla. En
    // Nueva York ningún día pierde 5000.
    let walk_days = |r: &Rules, days: &[(chrono::NaiveDate, Vec<Decimal>)]| {
        let mut refs = days.iter().map(|(d, xs)| DayRef {
            date: Some(*d),
            pnl: xs,
        });
        walk(r, &mut refs).phases[0]
    };
    let utc = rules(&rules_json());
    let s = walk_days(&utc, &group_days(&utc, &trades));
    assert_eq!(s.end, End::Failed(FailRule::DailyLoss));
    assert_eq!(s.end_day.unwrap().to_string(), "2026-11-26");
    let s = walk_days(&r, &days);
    assert_eq!(s.end, End::Unfinished);
    assert_eq!(s.trading_days, 2);
}
