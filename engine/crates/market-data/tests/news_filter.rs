//! Filtro de noticias de alto impacto (spec §7.2, #91) con casos construidos a mano.

use cfds_core::{Candle, EconomicEvent, Impact, Timeframe};
use cfds_market_data::{NewsFilterConfig, news_flags};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::dec;

const REAL: &str = include_str!("../../../../config/news_filter.toml");

fn cfg() -> NewsFilterConfig {
    NewsFilterConfig::from_toml(REAL).unwrap()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// Vela de DE40 que empieza a las 13:00: se decide y se entra a las 13:15 (τ).
fn de40() -> Candle {
    Candle {
        instrument_id: "DE40".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: at("2026-12-10T13:00:00Z"),
        open: dec!(24000),
        high: dec!(24010),
        low: dec!(23990),
        close: dec!(24005),
        volume: None,
    }
}

fn event(
    country: Option<&str>,
    instrument: Option<&str>,
    impact: Impact,
    at_: &str,
) -> EconomicEvent {
    EconomicEvent {
        source: "calendario".into(),
        event: "Decisión de tipos".into(),
        country: country.map(Into::into),
        instrument_id: instrument.map(|i| i.parse().unwrap()),
        impact,
        scheduled_at: at(at_),
        published_at: at("2026-12-01T08:00:00Z"),
    }
}

fn allows(events: &[EconomicEvent]) -> bool {
    news_flags(&[de40()], events, &cfg())[0]
}

#[test]
fn the_repository_config_is_the_spec() {
    let c = cfg();
    assert_eq!(c.window, TimeDelta::minutes(15));
    assert_eq!(c.blocking, vec![Impact::High]);
    assert_eq!(c.countries[&"DE40".parse().unwrap()], vec!["DE", "EU"]);
    assert_eq!(c.countries[&"US100".parse().unwrap()], vec!["US"]);
}

#[test]
fn a_high_impact_event_within_15_minutes_blocks_both_edges_included() {
    let hi = |t| event(Some("EU"), None, Impact::High, t);
    assert!(!allows(&[hi("2026-12-10T13:30:00Z")]), "+15 min");
    assert!(allows(&[hi("2026-12-10T13:30:01Z")]), "+15 min y 1 s");
    assert!(!allows(&[hi("2026-12-10T13:00:00Z")]), "−15 min");
    assert!(allows(&[hi("2026-12-10T12:59:59Z")]), "−15 min y 1 s");
    assert!(allows(&[]), "sin eventos");
}

#[test]
fn only_high_impact_blocks() {
    for impact in [Impact::Low, Impact::Medium] {
        assert!(allows(&[event(
            Some("DE"),
            None,
            impact,
            "2026-12-10T13:15:00Z"
        )]));
    }
    assert!(!allows(&[event(
        Some("DE"),
        None,
        Impact::High,
        "2026-12-10T13:15:00Z"
    )]));
}

#[test]
fn only_events_of_the_instrument_or_its_countries_count() {
    let at_tau = "2026-12-10T13:15:00Z";
    assert!(
        allows(&[event(Some("US"), None, Impact::High, at_tau)]),
        "otro país"
    );
    assert!(
        !allows(&[event(Some("DE"), None, Impact::High, at_tau)]),
        "su país"
    );
    assert!(
        !allows(&[event(None, Some("DE40"), Impact::High, at_tau)]),
        "el instrumento"
    );
    assert!(
        allows(&[event(None, Some("US100"), Impact::High, at_tau)]),
        "otro instrumento"
    );
    // Sin países configurados para el instrumento, solo cuentan sus eventos propios.
    let mut c = cfg();
    c.countries.remove(&"DE40".parse().unwrap());
    assert!(
        news_flags(
            &[de40()],
            &[event(Some("DE"), None, Impact::High, at_tau)],
            &c
        )[0]
    );
    assert!(
        !news_flags(
            &[de40()],
            &[event(None, Some("DE40"), Impact::High, at_tau)],
            &c
        )[0]
    );
}

#[test]
fn an_event_counts_only_once_published() {
    // Noticias con hora de publicación real (CLAUDE.md §5b): publicado después de τ, no existe.
    let mut late = event(Some("DE"), None, Impact::High, "2026-12-10T13:20:00Z");
    late.published_at = at("2026-12-10T13:15:01Z");
    assert!(allows(std::slice::from_ref(&late)));
    late.published_at = at("2026-12-10T13:15:00Z");
    assert!(!allows(&[late]));
}

#[test]
fn one_flag_per_candle_in_order() {
    let mut next = de40();
    next.start += TimeDelta::hours(1);
    let ev = [event(
        Some("EU"),
        None,
        Impact::High,
        "2026-12-10T13:15:00Z",
    )];
    assert_eq!(news_flags(&[de40(), next], &ev, &cfg()), vec![false, true]);
}

#[test]
fn invalid_configuration_is_rejected() {
    for (from, to) in [
        ("window_minutes = 15", "window_minutes = 0"),
        (r#"blocking_impacts = ["high"]"#, "blocking_impacts = []"),
        (
            r#"blocking_impacts = ["high"]"#,
            r#"blocking_impacts = ["critical"]"#,
        ),
        (r#"DE40 = ["DE", "EU"]"#, r#"DE40 = ["DEU"]"#),
        (r#"DE40 = ["DE", "EU"]"#, "DE40 = []"),
        (r#"DE40 = ["DE", "EU"]"#, r#"de40 = ["DE"]"#),
    ] {
        let text = REAL.replace(from, to);
        assert_ne!(text, REAL, "{from}");
        assert!(NewsFilterConfig::from_toml(&text).is_err(), "{to}");
    }
}
