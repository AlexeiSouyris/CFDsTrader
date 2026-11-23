//! Filtro de noticias de alto impacto (spec §7.2, #91) con casos construidos a mano.

use cfds_core::{Candle, EconomicEvent, Impact, Timeframe};
use cfds_market_data::{NewsFilter, NewsFilterConfig, calendar_coverage, news_flags};
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
    assert_eq!(c.calendar_stale_after, TimeDelta::days(8));
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
        (
            "calendar_stale_after_days = 8",
            "calendar_stale_after_days = 0",
        ),
    ] {
        let text = REAL.replace(from, to);
        assert_ne!(text, REAL, "{from}");
        assert!(NewsFilterConfig::from_toml(&text).is_err(), "{to}");
    }
}

/// Velas de DE40 de 15m seguidas desde `from`.
fn series(from: &str, n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| Candle {
            start: at(from) + TimeDelta::minutes(15 * i as i64),
            ..de40()
        })
        .collect()
}

#[test]
fn coverage_starts_at_the_first_reading_and_ends_after_a_long_silence() {
    let stale = cfg().calendar_stale_after;
    // Cierres a las 13:15, 13:30, 13:45 y 14:00.
    let c = series("2026-12-10T13:00:00Z", 4);
    // Sin lecturas: nada tiene calendario.
    assert_eq!(calendar_coverage(&c, &[], stale), vec![false; 4]);
    // La ingesta empezó a las 13:30: antes no hay calendario (no hay histórico).
    let first = [at("2026-12-10T13:30:00Z")];
    assert_eq!(
        calendar_coverage(&c, &first, stale),
        vec![false, true, true, true]
    );
    // Una lectura posterior al cierre no cuenta para esa vela (sin look-ahead).
    let later = [at("2026-12-10T13:50:00Z")];
    assert_eq!(
        calendar_coverage(&c, &later, stale),
        vec![false, false, false, true]
    );
    // Si la última lectura es de hace más de `calendar_stale_after`, la ingesta estuvo parada.
    let old = [at("2026-12-10T13:15:00Z") - stale - TimeDelta::minutes(1)];
    assert_eq!(
        calendar_coverage(&c, &old, stale),
        vec![false, false, false, false]
    );
    // El borde cuenta: justo `calendar_stale_after` antes del cierre todavía hay calendario.
    let last = &c[3..];
    let edge = [at("2026-12-10T14:00:00Z") - stale];
    assert_eq!(calendar_coverage(last, &edge, stale), vec![true]);
    let past = [edge[0] - TimeDelta::seconds(1)];
    assert_eq!(calendar_coverage(last, &past, stale), vec![false]);
}

#[test]
fn the_filter_reports_blocked_candles_and_uncovered_spans() {
    let c = series("2026-12-10T12:00:00Z", 8); // cierres de 12:15 a 14:00
    let ev = [event(
        Some("EU"),
        None,
        Impact::High,
        "2026-12-10T13:45:00Z",
    )];
    let sightings = [at("2026-12-01T08:00:00Z"), at("2026-12-10T12:40:00Z")];
    let mut cfg = cfg();
    cfg.calendar_stale_after = TimeDelta::days(3);
    let f = NewsFilter::new(&c, &ev, &sightings, &cfg);
    // El evento de las 13:45 bloquea las entradas de 13:30, 13:45 y 14:00.
    assert_eq!(
        f.allowed,
        vec![true, true, true, true, true, false, false, false]
    );
    assert_eq!(f.blocked(), 3);
    // Del 1 al 10 hubo más de 3 días sin lecturas: hasta la lectura de las 12:40, sin calendario.
    assert_eq!(f.uncovered(), 2);
    assert_eq!(
        f.uncovered_spans(&c),
        vec![(at("2026-12-10T12:00:00Z"), at("2026-12-10T12:30:00Z"))]
    );
}
