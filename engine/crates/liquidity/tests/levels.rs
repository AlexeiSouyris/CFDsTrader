//! Mapa de liquidez (spec §5.4.1, #231) con series construidas a mano: sin repintado, sin
//! look-ahead, fin de semana, velas faltantes, barrido y roto en la misma vela, y calentamiento.
//!
//! Noviembre de 2026: Nueva York (XNYS) abre 14:30–21:00 UTC (EST).

use std::collections::HashMap;
use std::path::Path;

use cfds_core::{
    Candle, LiquidityLevel, LiquidityLevelKind as Kind, LiquidityLevelState as State,
    LiquiditySession, Timeframe,
};
use cfds_indicators::wilder_atr;
use cfds_liquidity::{LiquidityConfig, is_active_at, liquidity_levels};
use cfds_market_data::{ExchangeCalendar, Session};
use cfds_patterns::{PatternsConfig, confirmed_pivots};
use chrono::{DateTime, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn cfg() -> LiquidityConfig {
    LiquidityConfig::from_toml(include_str!("../../../../config/liquidity.toml")).unwrap()
}

fn nyse() -> Session {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../config");
    ExchangeCalendar::load(&dir, HashMap::new())
        .unwrap()
        .session("XNYS")
        .unwrap()
        .clone()
}

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn bar(start: DateTime<Utc>, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// `n` velas de 15m seguidas desde `from`: O = C = 100, H = 101, L = 99.
fn flat(from: &str, n: usize) -> Vec<Candle> {
    (0..n)
        .map(|i| {
            let start = at(from) + TimeDelta::minutes(15 * i as i64);
            bar(start, dec!(100), dec!(101), dec!(99), dec!(100))
        })
        .collect()
}

/// Cambia la vela que empieza en `start`.
fn set(c: &mut [Candle], start: &str, h: Decimal, l: Decimal, close: Decimal) {
    let k = c.iter_mut().find(|k| k.start == at(start)).unwrap();
    (k.high, k.low, k.close) = (h, l, close);
}

fn levels_with(c: &[Candle], cfg: &LiquidityConfig) -> Vec<LiquidityLevel> {
    let strength = PatternsConfig::from_toml(include_str!("../../../../config/patterns.toml"))
        .unwrap()
        .pivots
        .strength;
    let atr = wilder_atr(c, cfg.atr_period);
    let pivots = confirmed_pivots(c, strength);
    liquidity_levels(c, &atr, &pivots, &nyse(), &cfg.levels)
}

fn levels(c: &[Candle]) -> Vec<LiquidityLevel> {
    levels_with(c, &cfg())
}

fn of(levels: &[LiquidityLevel], kind: Kind) -> Vec<&LiquidityLevel> {
    levels.iter().filter(|l| l.kind == kind).collect()
}

/// Lunes 16 y martes 17: un pico a 110 y un mínimo a 90 dentro de la sesión del lunes.
fn monday_tuesday() -> Vec<Candle> {
    let mut c = flat("2026-11-16T00:00:00Z", 2 * 96);
    set(
        &mut c,
        "2026-11-16T16:00:00Z",
        dec!(110),
        dec!(99),
        dec!(100),
    );
    set(
        &mut c,
        "2026-11-16T18:00:00Z",
        dec!(101),
        dec!(90),
        dec!(100),
    );
    c
}

#[test]
fn pdh_and_pdl_come_from_the_exchange_session_and_exist_from_its_close() {
    let all = levels(&monday_tuesday());
    let pdh = of(&all, Kind::Pdh);
    let pdl = of(&all, Kind::Pdl);
    // Lunes (con el pico) y martes (plano).
    assert_eq!(pdh.len(), 2);
    assert_eq!(
        (pdh[0].price, pdh[0].exists_from, pdh[0].state),
        (dec!(110), at("2026-11-16T21:00:00Z"), State::Intact)
    );
    assert_eq!((pdl[0].price, pdl[0].session), (dec!(90), None));
    assert_eq!(pdh[1].price, dec!(101));
    // La semana no terminó: no hay PWH ni PWL.
    assert!(of(&all, Kind::Pwh).is_empty() && of(&all, Kind::Pwl).is_empty());
    // Las ventanas UTC: Nueva York (13:30–20:00) tiene el pico y el mínimo; Asia, no.
    let ny_high = all
        .iter()
        .find(|l| l.kind == Kind::SessionHigh && l.session == Some(LiquiditySession::Ny))
        .unwrap();
    assert_eq!(
        (ny_high.price, ny_high.exists_from),
        (dec!(110), at("2026-11-16T20:00:00Z"))
    );
    let asia_high = all
        .iter()
        .find(|l| l.kind == Kind::SessionHigh && l.session == Some(LiquiditySession::Asia))
        .unwrap();
    assert_eq!(
        (asia_high.price, asia_high.exists_from),
        (dec!(101), at("2026-11-16T07:00:00Z"))
    );
}

#[test]
fn a_level_does_not_exist_before_its_time() {
    let c = monday_tuesday();
    let close = at("2026-11-16T21:00:00Z");
    // Hasta la vela de 20:30 (cierra 20:45), la sesión del lunes no terminó.
    let before: Vec<_> = c.iter().take_while(|k| k.end() < close).cloned().collect();
    assert!(of(&levels(&before), Kind::Pdh).is_empty());
    // Con la vela de 20:45 (cierra 21:00), ya existe.
    let upto: Vec<_> = c.iter().take_while(|k| k.end() <= close).cloned().collect();
    assert_eq!(of(&levels(&upto), Kind::Pdh)[0].price, dec!(110));
    // Ningún nivel existe después del final de la serie que lo calculó.
    for n in 1..c.len() {
        let now = c[n - 1].end();
        assert!(levels(&c[..n]).iter().all(|l| l.exists_from <= now));
    }
}

#[test]
fn swept_closes_back_inside_and_broken_closes_beyond() {
    let mut c = monday_tuesday();
    // Martes 15:00: pincha el PDH (110) y cierra abajo → barrido.
    set(
        &mut c,
        "2026-11-17T15:00:00Z",
        dec!(112),
        dec!(99),
        dec!(105),
    );
    // Martes 16:00: cierra arriba de 110. Ya estaba barrido: no cambia.
    set(
        &mut c,
        "2026-11-17T16:00:00Z",
        dec!(116),
        dec!(99),
        dec!(115),
    );
    // Martes 17:00: perfora el PDL (90) y cierra abajo → roto.
    set(
        &mut c,
        "2026-11-17T17:00:00Z",
        dec!(101),
        dec!(85),
        dec!(88),
    );
    let all = levels(&c);
    let pdh = of(&all, Kind::Pdh)[0];
    assert_eq!(
        (pdh.state, pdh.state_changed_at),
        (State::Swept, Some(at("2026-11-17T15:15:00Z")))
    );
    let pdl = of(&all, Kind::Pdl)[0];
    assert_eq!(
        (pdl.state, pdl.state_changed_at),
        (State::Broken, Some(at("2026-11-17T17:15:00Z")))
    );
}

#[test]
fn a_candle_that_pierces_and_closes_beyond_breaks_the_level_without_sweeping_it() {
    let mut c = monday_tuesday();
    // La misma vela pasa el nivel (como un sweep) y cierra del otro lado: cuenta como roto.
    set(
        &mut c,
        "2026-11-17T15:00:00Z",
        dec!(120),
        dec!(99),
        dec!(111),
    );
    // Después vuelve abajo: el estado ya quedó fijo.
    set(
        &mut c,
        "2026-11-17T15:15:00Z",
        dec!(112),
        dec!(99),
        dec!(100),
    );
    let pdh = levels(&c)
        .into_iter()
        .find(|l| l.kind == Kind::Pdh && l.price == dec!(110))
        .unwrap();
    assert_eq!(
        (pdh.state, pdh.state_changed_at),
        (State::Broken, Some(at("2026-11-17T15:15:00Z")))
    );
    // Una vela que barre el PDH y el PDL a la vez cambia los dos.
    let mut c = monday_tuesday();
    set(
        &mut c,
        "2026-11-17T15:00:00Z",
        dec!(115),
        dec!(85),
        dec!(100),
    );
    let all = levels(&c);
    for kind in [Kind::Pdh, Kind::Pdl] {
        assert_eq!(of(&all, kind)[0].state, State::Swept, "{kind:?}");
    }
}

#[test]
fn weekend_gap_breaks_friday_levels_at_the_first_monday_candle() {
    // Lunes 16 a viernes 20 hasta las 22:00 UTC, y el lunes 23 desde las 00:00, más arriba.
    let mut c = flat("2026-11-16T00:00:00Z", 4 * 96 + 88);
    set(
        &mut c,
        "2026-11-18T15:00:00Z",
        dec!(108),
        dec!(99),
        dec!(100),
    );
    let monday: Vec<Candle> = (0..8)
        .map(|i| {
            let start = at("2026-11-23T00:00:00Z") + TimeDelta::minutes(15 * i);
            bar(start, dec!(120), dec!(121), dec!(119), dec!(120))
        })
        .collect();
    let friday_only = levels(&c);
    c.extend(monday);
    let all = levels(&c);

    let pwh = of(&all, Kind::Pwh);
    assert_eq!(pwh.len(), 1);
    assert_eq!(
        (pwh[0].price, pwh[0].exists_from),
        (dec!(108), at("2026-11-20T21:00:00Z"))
    );
    // El gap: la primera vela del lunes (cierra 00:15) rompe todos los altos que seguían activos.
    let monday_first_close = at("2026-11-23T00:15:00Z");
    assert_eq!(
        (pwh[0].state, pwh[0].state_changed_at),
        (State::Broken, Some(monday_first_close))
    );
    let friday_pdh = of(&all, Kind::Pdh).into_iter().last().unwrap();
    assert_eq!(friday_pdh.exists_from, at("2026-11-20T21:00:00Z"));
    assert_eq!(friday_pdh.state_changed_at, Some(monday_first_close));
    // El fin de semana no tiene velas: no hay niveles nacidos el sábado o el domingo.
    assert!(
        all.iter()
            .all(|l| l.exists_from <= at("2026-11-20T22:00:00Z")
                || l.exists_from > at("2026-11-23T00:00:00Z"))
    );
    // Lo que ya existía el viernes es igual, salvo el estado que cambió el lunes.
    for (a, b) in friday_only.iter().zip(&all) {
        assert_eq!(
            (a.kind, a.price, a.exists_from),
            (b.kind, b.price, b.exists_from)
        );
    }
}

#[test]
fn missing_candles_use_what_there_is_and_the_level_waits_for_the_window_end() {
    let mut c = monday_tuesday();
    // Faltan la vela del pico de las 16:00 y la última de la sesión (20:45).
    c.retain(|k| k.start != at("2026-11-16T16:00:00Z") && k.start != at("2026-11-16T20:45:00Z"));
    let all = levels(&c);
    let pdh = of(&all, Kind::Pdh)[0];
    assert_eq!(
        (pdh.price, pdh.exists_from),
        (dec!(101), at("2026-11-16T21:00:00Z"))
    );
    // Con datos hasta las 20:45 todavía no se sabe si la sesión terminó: no existe.
    let upto: Vec<_> = c
        .iter()
        .take_while(|k| k.end() <= at("2026-11-16T20:45:00Z"))
        .cloned()
        .collect();
    assert!(of(&levels(&upto), Kind::Pdh).is_empty());
}

#[test]
fn warmup_needs_a_valid_atr_a_complete_period_and_lookback_bars_for_equal_levels() {
    // Sin ATR válido no existe nada.
    let c = monday_tuesday();
    let cfg = cfg();
    let pivots = confirmed_pivots(&c, 3);
    assert!(liquidity_levels(&c, &vec![None; c.len()], &pivots, &nyse(), &cfg.levels).is_empty());
    // Si la serie empieza después de la apertura, esa sesión está incompleta y no cuenta.
    let late: Vec<_> = c
        .iter()
        .filter(|k| k.start >= at("2026-11-16T15:00:00Z"))
        .cloned()
        .collect();
    let pdh = of(&levels(&late), Kind::Pdh)
        .into_iter()
        .map(|l| l.exists_from)
        .collect::<Vec<_>>();
    assert_eq!(pdh, vec![at("2026-11-17T21:00:00Z")]);
    // EQH: dos máximos iguales, pero sin `lookback_bars` velas todavía no existe.
    let eq = equal_highs_series();
    let eqh_at = |n: usize| of(&levels(&eq[..n]), Kind::Eqh).len();
    assert!(eqh_at(eq.len()) > 0);
    let short = LiquidityConfig {
        levels: cfds_liquidity::LevelsConfig {
            equal_lookback_bars: eq.len() + 1,
            ..cfg.levels
        },
        ..cfg
    };
    assert!(of(&levels_with(&eq, &short), Kind::Eqh).is_empty());
}

/// Dos picos a 120 y 120,05 (dentro de 0,10 × ATR) después de 200 velas onduladas.
fn equal_highs_series() -> Vec<Candle> {
    let mut c = flat("2026-11-16T00:00:00Z", 230);
    for (i, k) in c.iter_mut().enumerate() {
        // Velas alternadas para tener ATR, sin pivotes (los vecinos empatan).
        let up = Decimal::from((i % 2) as i64);
        (k.high, k.low) = (dec!(101) + up, dec!(99) - up);
    }
    let peak = |c: &mut [Candle], i: usize, h: Decimal| {
        c[i].high = h;
        c[i].close = dec!(100);
    };
    peak(&mut c, 210, dec!(120));
    peak(&mut c, 220, dec!(120.05));
    c
}

#[test]
fn equal_highs_take_the_highest_and_exist_from_the_last_confirmation() {
    let c = equal_highs_series();
    let all = levels(&c);
    let eqh = of(&all, Kind::Eqh);
    assert_eq!(eqh.len(), 1);
    // El segundo pico (vela 220) se confirma en la 223: existe desde su cierre.
    assert_eq!(
        (eqh[0].price, eqh[0].exists_from, eqh[0].session),
        (dec!(120.05), c[223].end(), None)
    );
    // Con tolerancia más chica, ya no son iguales.
    let tight = LiquidityConfig {
        levels: cfds_liquidity::LevelsConfig {
            equal_tolerance_atr: dec!(0.001),
            ..cfg().levels
        },
        ..cfg()
    };
    assert!(of(&levels_with(&c, &tight), Kind::Eqh).is_empty());
}

#[test]
fn a_level_expires_after_max_age_bars_without_changing_its_state() {
    let mut c = monday_tuesday();
    let cfg = cfg();
    let mut more = flat("2026-11-18T00:00:00Z", cfg.levels.max_age_bars);
    // Supera el PDH del lunes recién después de max_age_bars velas: ya no cuenta.
    let last = more.len() - 1;
    more[last].high = dec!(130);
    c.extend(more);
    let all = levels(&c);
    let pdh = of(&all, Kind::Pdh)[0];
    let born = c.iter().position(|k| k.end() == pdh.exists_from).unwrap();
    // Vence al cierre de la vela número max_age_bars desde que existe.
    let expiry = born + cfg.levels.max_age_bars;
    assert_eq!(
        (pdh.price, pdh.state, pdh.state_changed_at, pdh.expired_at),
        (dec!(110), State::Intact, None, Some(c[expiry].end()))
    );
    assert!(!is_active_at(pdh, c[born - 1].end()));
    assert!(is_active_at(pdh, c[born].end()));
    assert!(is_active_at(pdh, c[expiry - 1].end()));
    assert!(!is_active_at(pdh, c[expiry].end()));
    // Con una vela menos todavía no venció.
    let before = levels(&c[..expiry]);
    assert_eq!(of(&before, Kind::Pdh)[0].expired_at, None);
    // Un nivel barrido o roto no vence.
    assert!(
        all.iter()
            .all(|l| l.state == State::Intact || l.expired_at.is_none())
    );
}

/// Serie de dos semanas con ondas, pivotes, máximos iguales, barridos y rupturas.
fn two_weeks() -> Vec<Candle> {
    let mut seed: u64 = 7;
    let mut noise = move || {
        seed = seed
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        Decimal::from((seed >> 33) % 7) / dec!(100)
    };
    (0..13 * 96)
        .map(|i| {
            // Triángulo de período 40 entre 100 y 110, con una deriva lenta.
            let phase = (i % 40) as i64;
            let tri = Decimal::from(if phase < 20 { phase } else { 40 - phase }) / dec!(2);
            let drift = Decimal::from(i as i64 / 200) * dec!(3);
            let mid = dec!(100) + tri + drift;
            let (o, c) = (mid - dec!(0.2), mid + dec!(0.2));
            let start = at("2026-11-02T00:00:00Z") + TimeDelta::minutes(15 * i as i64);
            bar(
                start,
                o,
                c + dec!(0.3) + noise(),
                o - dec!(0.3) - noise(),
                c,
            )
        })
        .collect()
}

#[test]
fn adding_candles_never_repaints_the_past() {
    let c = two_weeks();
    let full = levels(&c);
    for kind in [
        Kind::Eqh,
        Kind::Eql,
        Kind::Pdh,
        Kind::Pwl,
        Kind::SessionHigh,
    ] {
        assert!(!of(&full, kind).is_empty(), "la serie no tiene {kind:?}");
    }
    assert!(full.iter().any(|l| l.state == State::Swept));
    assert!(full.iter().any(|l| l.state == State::Broken));
    assert!(full.iter().any(|l| l.expired_at.is_some()));
    for n in (1..c.len()).step_by(7) {
        let now = c[n - 1].end();
        let part = levels(&c[..n]);
        let expected: Vec<_> = full.iter().filter(|l| l.exists_from <= now).collect();
        assert_eq!(part.len(), expected.len(), "con {n} velas");
        for (p, f) in part.iter().zip(expected) {
            assert_eq!(
                (p.kind, p.session, p.price, p.exists_from),
                (f.kind, f.session, f.price, f.exists_from),
                "con {n} velas"
            );
            if p.state == State::Intact && p.expired_at.is_none() {
                // Lo único que puede pasar después es que cambie o venza con una vela posterior.
                assert!(f.state_changed_at.is_none_or(|t| t > now), "con {n} velas");
                assert!(f.expired_at.is_none_or(|t| t > now), "con {n} velas");
            } else {
                assert_eq!(p, f, "con {n} velas");
            }
        }
    }
}
