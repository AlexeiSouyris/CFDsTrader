//! Criterio de aprobación de la Fase 2: tests que fallan con datos corruptos a propósito.
//! Cada caso rompe UNA cosa de una serie sana y verifica que la validación lo detecta.

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{
    IssueKind, SessionCalendar, ValidationConfig, check_series, validate_series,
};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

/// Mercado siempre abierto: toda vela entre dos fechas debería existir.
struct AlwaysOpen;

impl SessionCalendar for AlwaysOpen {
    fn expected_starts(
        &self,
        _: &InstrumentId,
        tf: Timeframe,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Option<Vec<DateTime<Utc>>> {
        Some(
            std::iter::successors(Some(from), |t| Some(*t + tf.duration()))
                .take_while(|t| *t < to)
                .collect(),
        )
    }
}

/// Mercado cerrado entre dos velas cualesquiera (por ejemplo, de noche o un fin de semana).
struct ClosedInBetween;

impl SessionCalendar for ClosedInBetween {
    fn expected_starts(
        &self,
        _: &InstrumentId,
        _: Timeframe,
        _: DateTime<Utc>,
        _: DateTime<Utc>,
    ) -> Option<Vec<DateTime<Utc>>> {
        Some(Vec::new())
    }
}

fn cfg() -> ValidationConfig {
    ValidationConfig::from_toml(include_str!("../../../../config/data_validation.toml")).unwrap()
}

fn t0() -> DateTime<Utc> {
    "2026-11-19T14:30:00Z".parse().unwrap()
}

/// `n` velas de 15m contiguas y sanas: suben 1 punto por vela con rango de 10 (ATR = 10).
fn healthy(n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let base = dec!(26700) + Decimal::from(i);
            Candle {
                instrument_id: "US100".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0() + TimeDelta::minutes(15 * i as i64),
                open: base,
                high: base + dec!(6),
                low: base - dec!(4),
                close: base + dec!(1),
                volume: Some(dec!(500)),
            }
        })
        .collect()
}

fn kinds(candles: &[Candle], calendar: &impl SessionCalendar) -> Vec<(usize, IssueKind)> {
    validate_series(candles, calendar, &cfg())
        .into_iter()
        .map(|i| (i.index, i.kind))
        .collect()
}

#[test]
fn a_healthy_series_has_no_issues() {
    assert_eq!(kinds(&healthy(40), &AlwaysOpen), vec![]);
    assert_eq!(kinds(&[], &AlwaysOpen), vec![]);
}

#[test]
fn detects_zero_or_negative_prices() {
    let mut s = healthy(5);
    s[2].low = dec!(0);
    let found = kinds(&s, &AlwaysOpen);
    assert!(found.contains(&(
        2,
        IssueKind::NonPositivePrice {
            field: "low",
            value: dec!(0)
        }
    )));
}

#[test]
fn detects_negative_volume() {
    let mut s = healthy(5);
    s[3].volume = Some(dec!(-1));
    assert_eq!(
        kinds(&s, &AlwaysOpen),
        vec![(3, IssueKind::NegativeVolume(dec!(-1)))]
    );
}

#[test]
fn detects_inconsistent_ohlc() {
    let mut s = healthy(5);
    s[1].high = s[1].close - dec!(1); // el máximo quedó por debajo del cierre
    let found = kinds(&s, &AlwaysOpen);
    assert!(
        matches!(found.as_slice(), [(1, IssueKind::InconsistentOhlc { .. })]),
        "{found:?}"
    );
}

#[test]
fn detects_candles_off_the_15_minute_grid() {
    let mut s = healthy(1);
    s[0].start += TimeDelta::minutes(7);
    assert_eq!(
        kinds(&s, &AlwaysOpen),
        vec![(0, IssueKind::OffGrid { grid_minutes: 15 })]
    );
}

#[test]
fn detects_duplicates() {
    let mut s = healthy(4);
    s.insert(2, s[1].clone());
    assert_eq!(kinds(&s, &AlwaysOpen), vec![(2, IssueKind::Duplicate)]);
}

#[test]
fn detects_broken_time_order() {
    let mut s = healthy(4);
    s.swap(1, 2);
    let found = kinds(&s, &AlwaysOpen);
    assert!(
        found
            .iter()
            .any(|(i, k)| *i == 2 && matches!(k, IssueKind::OutOfOrder { .. })),
        "{found:?}"
    );
}

#[test]
fn detects_overlapping_hourly_candles() {
    let mut s = healthy(2);
    for c in &mut s {
        c.timeframe = Timeframe::H1;
    }
    s[1].start = s[0].start + TimeDelta::minutes(30); // empieza antes de que cierre la anterior
    let found = kinds(&s, &AlwaysOpen);
    assert!(
        matches!(found.as_slice(), [(1, IssueKind::Overlap { .. })]),
        "{found:?}"
    );
}

#[test]
fn detects_missing_candles_inside_the_session() {
    let mut s = healthy(6);
    s.drain(2..4); // faltan las velas de las 15:00 y las 15:15
    assert_eq!(
        kinds(&s, &AlwaysOpen),
        vec![(
            2,
            IssueKind::MissingCandles {
                count: 2,
                first_missing: t0() + TimeDelta::minutes(30)
            }
        )]
    );
}

#[test]
fn a_gap_while_the_market_is_closed_is_not_an_issue() {
    let mut s = healthy(20);
    s.drain(5..15); // hueco, pero el calendario dice que el mercado estaba cerrado
    assert_eq!(kinds(&s, &ClosedInBetween), vec![]);
}

#[test]
fn detects_mixed_instruments() {
    let mut s = healthy(3);
    s[1].instrument_id = "DE40".parse().unwrap();
    let found = kinds(&s, &AlwaysOpen);
    assert!(
        matches!(found.as_slice(), [(1, IssueKind::MixedSeries { .. })]),
        "{found:?}"
    );
}

#[test]
fn detects_a_huge_jump_between_contiguous_candles() {
    let mut s = healthy(30);
    // ATR ≈ 10. Un salto de 1.000 puntos en la vela 20, sin corte de sesión, es sospechoso.
    let c = &mut s[20];
    c.open += dec!(1000);
    c.high += dec!(1000);
    c.low += dec!(1000);
    c.close += dec!(1000);
    let found = kinds(&s, &AlwaysOpen);
    assert!(
        matches!(found.first(), Some((20, IssueKind::Outlier { .. }))),
        "{found:?}"
    );
}

#[test]
fn a_normal_move_of_a_few_atr_is_not_an_outlier() {
    let mut s = healthy(30);
    s[20].high += dec!(40); // rango de 50 = 5 × ATR: grande, pero por debajo de 10 × ATR
    assert_eq!(kinds(&s, &AlwaysOpen), vec![]);
}

#[test]
fn a_big_jump_after_a_session_break_is_not_an_outlier() {
    let mut s = healthy(30);
    for c in &mut s[20..] {
        c.start += TimeDelta::hours(16); // el mercado cerró de noche entre la vela 19 y la 20
        c.open += dec!(1000);
        c.high += dec!(1000);
        c.low += dec!(1000);
        c.close += dec!(1000);
    }
    assert_eq!(kinds(&s, &ClosedInBetween), vec![]);
}

#[test]
fn no_outlier_check_during_atr_warmup() {
    // Con menos de 14 velas previas no hay ATR todavía: el salto no se puede juzgar y no se reporta.
    let mut s = healthy(10);
    s[5].high += dec!(5000);
    s[5].close += dec!(1000);
    let found = kinds(&s, &AlwaysOpen);
    assert!(
        found
            .iter()
            .all(|(_, k)| !matches!(k, IssueKind::Outlier { .. })),
        "{found:?}"
    );
}

// Spec §7.8 y §12: los datos sospechosos se descartan y queda registrado desde cuándo hubo
// un error de datos, para no abrir operaciones nuevas en ese instrumento.

#[test]
fn a_healthy_series_is_kept_whole_without_data_errors() {
    let c = healthy(30);
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert_eq!(checked.candles, c);
    assert!(checked.issues.is_empty() && checked.data_errors.is_empty());
}

#[test]
fn suspicious_candles_are_discarded_and_mark_a_data_error() {
    let mut c = healthy(30);
    c[10].low = dec!(0);
    c.insert(20, c[19].clone()); // duplicada
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert_eq!(checked.candles.len(), 29);
    assert!(!checked.candles.contains(&c[10]));
    // De la duplicada queda una sola copia.
    assert_eq!(checked.candles.iter().filter(|x| **x == c[19]).count(), 1);
    assert_eq!(checked.data_errors, vec![c[10].start, c[19].start]);
}

#[test]
fn a_gap_marks_the_first_missing_candle_and_keeps_the_next_one() {
    let mut c = healthy(30);
    let missing = c[12].start;
    c.remove(12);
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert_eq!(checked.candles, c, "la vela después del hueco sirve");
    assert_eq!(checked.data_errors, vec![missing]);
    // Con el mercado cerrado en el medio no falta nada: no es un error.
    assert!(
        check_series(&c, &ClosedInBetween, &cfg())
            .data_errors
            .is_empty()
    );
}

#[test]
fn a_candle_with_several_problems_is_one_data_error() {
    // Precio 0 y, con el ATR ya calculado, también un salto sospechoso: una sola marca.
    let mut c = healthy(30);
    c[25].low = dec!(0);
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert!(checked.issues.iter().filter(|i| i.index == 25).count() >= 2);
    assert_eq!(checked.data_errors, vec![c[25].start]);
    assert_eq!(checked.candles.len(), 29);
}
