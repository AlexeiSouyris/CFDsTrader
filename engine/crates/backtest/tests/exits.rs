//! Una prueba por cada regla de salida (spec §8 y §12), construidas a mano, y el espejo de
//! todas para cortos (#51).
//!
//! Escenario base (largo): señal en la vela 0, entrada en la apertura de la 1 a 100, stop 98
//! (R = 2), TP1 = 104, +1R = 102.

use cfds_backtest::{ExitConfig, ExitReason, Fill, Outcome, Trade, simulate};
use cfds_core::{Candle, Timeframe};
use cfds_strategy::Side;
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn cfg() -> ExitConfig {
    ExitConfig::from_toml(include_str!("../../../../config/backtest.toml")).unwrap()
}

fn bar(i: usize, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    let t0: DateTime<Utc> = "2026-11-19T14:30:00Z".parse().unwrap();
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0 + TimeDelta::minutes(15 * i as i64),
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// Velas a partir de la 1 (la 0 es la vela de la señal, neutra).
fn series(rows: &[(Decimal, Decimal, Decimal, Decimal)]) -> Vec<Candle> {
    std::iter::once(bar(0, dec!(100), dec!(100.5), dec!(99.5), dec!(100)))
        .chain(
            rows.iter()
                .enumerate()
                .map(|(k, &(o, h, l, c))| bar(k + 1, o, h, l, c)),
        )
        .collect()
}

fn long() -> Trade {
    Trade {
        side: Side::Long,
        entry_index: 1,
        entry: dec!(100),
        stop: dec!(98),
    }
}

fn atr(n: usize, v: Decimal) -> Vec<Option<Decimal>> {
    vec![Some(v); n]
}

fn run(candles: &[Candle], atr_value: Decimal, force: &[bool]) -> Outcome {
    let out = simulate(
        &long(),
        candles,
        &atr(candles.len(), atr_value),
        force,
        &cfg(),
    )
    .unwrap();
    // Cada escenario se verifica también en espejo: el corto tiene que dar lo mismo.
    assert_mirror(candles, atr_value, force, &out);
    out
}

fn assert_mirror(candles: &[Candle], atr_value: Decimal, force: &[bool], long_out: &Outcome) {
    let m = |x: Decimal| dec!(200) - x;
    let mirrored: Vec<Candle> = candles
        .iter()
        .enumerate()
        .map(|(i, c)| bar(i, m(c.open), m(c.low), m(c.high), m(c.close)))
        .collect();
    let short = Trade {
        side: Side::Short,
        entry_index: 1,
        entry: dec!(100),
        stop: dec!(102),
    };
    let out = simulate(
        &short,
        &mirrored,
        &atr(candles.len(), atr_value),
        force,
        &cfg(),
    )
    .unwrap();
    let expected: Vec<Fill> = long_out
        .fills
        .iter()
        .map(|f| Fill {
            price: m(f.price),
            ..*f
        })
        .collect();
    assert_eq!(out.fills, expected);
    assert_eq!(out.open_fraction, long_out.open_fraction);
    assert_eq!(
        out.realized_points(&short),
        long_out.realized_points(&long())
    );
}

fn fill(index: usize, price: Decimal, fraction: Decimal, reason: ExitReason) -> Fill {
    Fill {
        index,
        price,
        fraction,
        reason,
    }
}

const FLAT: (Decimal, Decimal, Decimal, Decimal) = (dec!(100), dec!(101.5), dec!(99), dec!(100.5));

#[test]
fn the_stop_can_be_hit_in_the_entry_candle() {
    let out = run(
        &series(&[(dec!(100), dec!(101), dec!(97.5), dec!(98))]),
        dec!(1),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![fill(1, dec!(98), dec!(1), ExitReason::Stop)]
    );
    assert_eq!(out.realized_points(&long()), dec!(-2));
}

#[test]
fn a_gap_through_the_stop_fills_at_the_open() {
    let out = run(
        &series(&[FLAT, (dec!(96), dec!(97), dec!(95), dec!(96))]),
        dec!(1),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![fill(2, dec!(96), dec!(1), ExitReason::Stop)]
    );
    assert_eq!(out.realized_points(&long()), dec!(-4));
}

#[test]
fn stop_and_tp_in_the_same_candle_assume_the_stop() {
    let out = run(
        &series(&[(dec!(100), dec!(105), dec!(97), dec!(103))]),
        dec!(1),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![fill(1, dec!(98), dec!(1), ExitReason::Stop)]
    );
}

#[test]
fn tp1_closes_half_and_moves_the_stop_to_the_entry() {
    // ATR grande: el trailing (104,5 − 10) no supera la entrada.
    let out = run(
        &series(&[
            (dec!(100), dec!(104.5), dec!(100.5), dec!(104)),
            (dec!(103), dec!(103), dec!(99.9), dec!(100.2)),
        ]),
        dec!(5),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![
            fill(1, dec!(104), dec!(0.5), ExitReason::Tp1),
            fill(2, dec!(100), dec!(0.5), ExitReason::BreakEven),
        ]
    );
    assert_eq!(out.realized_points(&long()), dec!(2));
}

#[test]
fn back_to_the_entry_in_the_tp1_candle_closes_the_rest_there() {
    // Entra a 100, baja a 99,5 (sin tocar el stop), sube a 104 y vuelve: no se sabe el orden
    // de los movimientos, así que se asume lo peor después del TP: vuelta a la entrada.
    let out = run(
        &series(&[(dec!(100), dec!(104), dec!(99.5), dec!(101))]),
        dec!(1),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![
            fill(1, dec!(104), dec!(0.5), ExitReason::Tp1),
            fill(1, dec!(100), dec!(0.5), ExitReason::BreakEven),
        ]
    );
}

#[test]
fn a_gap_through_tp1_fills_at_the_open() {
    let out = run(
        &series(&[FLAT, (dec!(106), dec!(107), dec!(105), dec!(106.5))]),
        dec!(5),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![fill(2, dec!(106), dec!(0.5), ExitReason::Tp1)]
    );
    assert_eq!(out.open_fraction, dec!(0.5));
}

#[test]
fn the_trailing_stop_follows_the_high_minus_two_atr() {
    let out = run(
        &series(&[
            // TP1 a 104; al cierre: trailing = 104,5 − 2 = 102,5.
            (dec!(100), dec!(104.5), dec!(100.5), dec!(104)),
            // Máximo 108: trailing = 106 (vale desde la vela siguiente, no en esta).
            (dec!(104), dec!(108), dec!(103), dec!(107.5)),
            // Toca 106.
            (dec!(107), dec!(107.5), dec!(105.5), dec!(106)),
        ]),
        dec!(1),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![
            fill(1, dec!(104), dec!(0.5), ExitReason::Tp1),
            fill(3, dec!(106), dec!(0.5), ExitReason::Trailing),
        ]
    );
    assert_eq!(out.realized_points(&long()), dec!(5));
}

#[test]
fn the_trailing_stop_never_moves_back() {
    let out = run(
        &series(&[
            (dec!(100), dec!(104.5), dec!(100.5), dec!(104)),
            (dec!(104), dec!(108), dec!(106.5), dec!(107.5)),
            (dec!(107), dec!(107.5), dec!(106.2), dec!(107)),
        ]),
        dec!(1),
        &[],
    );
    // El trailing quedó en 106 después de la vela 2; la 3 no lo toca.
    assert_eq!(out.fills.len(), 1);
    // Con un ATR que crece, el trailing calculado bajaría, pero el stop no retrocede.
    let mut atrs = atr(4, dec!(1));
    atrs[2] = Some(dec!(10));
    let candles = series(&[
        (dec!(100), dec!(104.5), dec!(100.5), dec!(104)),
        (dec!(104), dec!(108), dec!(106.5), dec!(107.5)),
        (dec!(107), dec!(107.5), dec!(102.4), dec!(103)),
    ]);
    let out = simulate(&long(), &candles, &atrs, &[], &cfg()).unwrap();
    assert_eq!(
        out.fills[1],
        fill(3, dec!(102.5), dec!(0.5), ExitReason::Trailing)
    );
}

#[test]
fn without_1r_after_32_candles_it_closes_at_the_close() {
    let out = run(&series(&[FLAT; 32]), dec!(1), &[]);
    assert_eq!(
        out.fills,
        vec![fill(32, dec!(100.5), dec!(1), ExitReason::Time)]
    );
    // A las 31 velas sigue abierta.
    let out = run(&series(&[FLAT; 31]), dec!(1), &[]);
    assert_eq!((out.fills.len(), out.open_fraction), (0, dec!(1)));
}

#[test]
fn touching_1r_avoids_the_time_exit() {
    let mut rows = vec![FLAT; 40];
    rows[5] = (dec!(100), dec!(102), dec!(99), dec!(100.5));
    let out = run(&series(&rows), dec!(1), &[]);
    assert_eq!((out.fills.len(), out.open_fraction), (0, dec!(1)));
}

#[test]
fn forced_close_on_friday_or_session_end() {
    let mut force = vec![false; 5];
    force[3] = true;
    let out = run(&series(&[FLAT; 4]), dec!(1), &force);
    assert_eq!(
        out.fills,
        vec![fill(3, dec!(100.5), dec!(1), ExitReason::Forced)]
    );
}

#[test]
fn the_stop_comes_before_a_forced_close() {
    let force = vec![true; 3];
    let out = run(
        &series(&[(dec!(100), dec!(101), dec!(97), dec!(99))]),
        dec!(1),
        &force,
    );
    assert_eq!(
        out.fills,
        vec![fill(1, dec!(98), dec!(1), ExitReason::Stop)]
    );
}

#[test]
fn still_open_when_the_data_ends() {
    let out = run(&series(&[FLAT; 3]), dec!(1), &[]);
    assert_eq!(out.fills, vec![]);
    assert_eq!(out.open_fraction, dec!(1));
    assert_eq!(out.realized_points(&long()), dec!(0));
}

#[test]
fn a_stop_on_the_wrong_side_is_rejected() {
    let c = series(&[FLAT]);
    let bad_long = Trade {
        stop: dec!(100),
        ..long()
    };
    assert_eq!(simulate(&bad_long, &c, &atr(2, dec!(1)), &[], &cfg()), None);
    let bad_short = Trade {
        side: Side::Short,
        stop: dec!(99),
        ..long()
    };
    assert_eq!(
        simulate(&bad_short, &c, &atr(2, dec!(1)), &[], &cfg()),
        None
    );
}

#[test]
fn candles_before_the_entry_are_ignored() {
    // La vela 0 (la de la señal) toca el stop, pero la operación todavía no existía.
    let mut c = series(&[FLAT; 2]);
    c[0] = bar(0, dec!(100), dec!(100.5), dec!(97), dec!(100));
    let out = simulate(&long(), &c, &atr(3, dec!(1)), &[], &cfg()).unwrap();
    assert_eq!(out.fills, vec![]);
}

#[test]
fn touching_the_stop_exactly_closes() {
    let out = run(
        &series(&[(dec!(100), dec!(101), dec!(98), dec!(99))]),
        dec!(1),
        &[],
    );
    assert_eq!(
        out.fills,
        vec![fill(1, dec!(98), dec!(1), ExitReason::Stop)]
    );
}

#[test]
fn the_trailing_uses_the_highest_high_since_tp1_not_the_last_one() {
    // El máximo 108 queda atrás; con un ATR que se achica, el trailing sale de 108, no de 107,5.
    let candles = series(&[
        (dec!(100), dec!(104.5), dec!(100.5), dec!(104)),
        (dec!(104), dec!(108), dec!(103), dec!(107)),
        (dec!(107), dec!(107.5), dec!(105.5), dec!(107)),
        (dec!(107.6), dec!(108), dec!(107.2), dec!(107.8)),
    ]);
    let atrs = vec![
        Some(dec!(1)),
        Some(dec!(1)),
        Some(dec!(3)),
        Some(dec!(0.25)),
        Some(dec!(0.25)),
    ];
    let out = simulate(&long(), &candles, &atrs, &[], &cfg()).unwrap();
    // Después de la vela 3: trailing = 108 − 2 × 0,25 = 107,5; la vela 4 lo toca.
    assert_eq!(
        out.fills[1],
        fill(4, dec!(107.5), dec!(0.5), ExitReason::Trailing)
    );
}
