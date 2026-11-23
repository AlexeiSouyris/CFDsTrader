//! Criterio de aprobación de la Fase 2: tests que fallan con datos corruptos a propósito.
//! Cada caso rompe UNA cosa de una serie sana y verifica que la validación lo detecta.

use cfds_core::{Candle, InstrumentId, Timeframe};
use cfds_market_data::{
    IssueKind, JumpOutcome, SessionCalendar, ValidationConfig, check_series, jump_outcome,
    validate_series,
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

// #271: un salto de más de 10 ATR es un error de datos solo si fue un pico aislado. Si el precio
// se sostiene, es un movimiento real. Se decide recién `confirm_bars` velas después.

/// Sube toda la serie desde la vela `from` en `points` (un salto que se sostiene).
fn shifted_from(mut c: Vec<Candle>, from: usize, points: Decimal) -> Vec<Candle> {
    for k in &mut c[from..] {
        k.open += points;
        k.high += points;
        k.low += points;
        k.close += points;
    }
    c
}

#[test]
fn a_spike_that_comes_back_is_a_data_error_and_the_candle_is_discarded() {
    // Un tick malo: la vela 20 llega a +1000 pero cierra donde venía (vuelve en la misma vela).
    let mut c = healthy(30);
    c[20].high += dec!(1000);
    assert_eq!(jump_outcome(&c, 20, &cfg()), JumpOutcome::Spike);
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert!(!checked.candles.contains(&c[20]));
    assert_eq!(checked.data_errors, vec![c[20].start]);
    assert!(checked.volatility.is_empty());
    // Mientras no se confirmó (4 velas), no se decide.
    assert_eq!(checked.blocked, vec![(c[20].start, Some(c[24].start))]);
    assert!(!checked.decision_allowed(c[23].start));
    assert!(checked.decision_allowed(c[24].start));

    // Un pico que vuelve unas velas después: la vela 20 y la 21 arriba, la 22 vuelve.
    let mut c = shifted_from(healthy(30), 20, dec!(1000));
    for k in &mut c[22..] {
        k.open -= dec!(1000);
        k.high -= dec!(1000);
        k.low -= dec!(1000);
        k.close -= dec!(1000);
    }
    assert_eq!(jump_outcome(&c, 20, &cfg()), JumpOutcome::Spike);
}

#[test]
fn a_jump_that_holds_is_a_real_move_kept_without_restarting_the_warmup() {
    let c = shifted_from(healthy(30), 20, dec!(1000));
    assert_eq!(jump_outcome(&c, 20, &cfg()), JumpOutcome::Sustained);
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert_eq!(checked.candles, c, "ninguna vela se descarta");
    assert!(
        checked.data_errors.is_empty(),
        "no reinicia el calentamiento"
    );
    assert_eq!(checked.volatility, vec![c[20].start]);
    // Se informa igual (la validación reporta todo) y no se decide hasta confirmar.
    assert!(
        checked
            .issues
            .iter()
            .any(|i| i.index == 20 && matches!(i.kind, IssueKind::Outlier { .. }))
    );
    assert_eq!(checked.blocked, vec![(c[20].start, Some(c[24].start))]);
}

#[test]
fn until_confirmed_the_jump_is_pending_and_nothing_is_decided() {
    // Sin look-ahead: con menos de 4 velas después del salto todavía no se sabe.
    let full = shifted_from(healthy(30), 20, dec!(1000));
    for n in 21..=24 {
        let part = &full[..n];
        assert_eq!(jump_outcome(part, 20, &cfg()), JumpOutcome::Pending, "{n}");
        let checked = check_series(part, &AlwaysOpen, &cfg());
        assert!(checked.data_errors.is_empty() && checked.volatility.is_empty());
        assert_eq!(checked.blocked, vec![(full[20].start, None)]);
        assert!(!checked.decision_allowed(full[n - 1].start));
    }
    // Con la vela de confirmación, sostenido; lo de antes no cambia.
    let checked = check_series(&full[..25], &AlwaysOpen, &cfg());
    assert_eq!(checked.volatility, vec![full[20].start]);
}

#[test]
fn a_news_day_jump_that_keeps_going_is_a_real_move() {
    // Datos inventados con la forma de un día de noticias (como el 9-4-2025 en DE40): una vela
    // con un rango de más de 10 ATR que cierra lejos y las siguientes que siguen más arriba.
    // No son precios reales: el repo no guarda precios.
    let mut c: Vec<Candle> = (0..30)
        .map(|i| {
            let base = dec!(10000) + Decimal::from(i % 3);
            Candle {
                instrument_id: "DE40".parse().unwrap(),
                timeframe: Timeframe::M15,
                start: t0() + TimeDelta::minutes(15 * i as i64),
                open: base,
                high: base + dec!(15),
                low: base - dec!(15),
                close: base,
                volume: Some(dec!(300)),
            }
        })
        .collect();
    let jump = [
        (dec!(10002), dec!(10600), dec!(9950), dec!(10350)),
        (dec!(10355), dec!(10700), dec!(10340), dec!(10550)),
        (dec!(10560), dec!(10650), dec!(10520), dec!(10640)),
        (dec!(10640), dec!(10760), dec!(10630), dec!(10720)),
        (dec!(10725), dec!(10760), dec!(10680), dec!(10730)),
    ];
    let last = c[29].start;
    for (k, (o, h, l, cl)) in jump.into_iter().enumerate() {
        c.push(Candle {
            start: last + TimeDelta::minutes(15 * (k as i64 + 1)),
            open: o,
            high: h,
            low: l,
            close: cl,
            ..c[0].clone()
        });
    }
    let at = 30;
    assert!(
        kinds(&c, &AlwaysOpen)
            .iter()
            .any(|(i, k)| *i == at && matches!(k, IssueKind::Outlier { .. }))
    );
    assert_eq!(jump_outcome(&c, at, &cfg()), JumpOutcome::Sustained);
    let checked = check_series(&c, &AlwaysOpen, &cfg());
    assert_eq!(checked.candles.len(), c.len());
    assert!(checked.data_errors.is_empty());
    assert_eq!(checked.volatility, vec![c[at].start]);
}
