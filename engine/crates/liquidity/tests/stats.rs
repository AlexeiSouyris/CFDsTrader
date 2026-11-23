//! Estadísticas de FVG y gaps de apertura de la spec §5.4.1 (#231), con series hechas a mano.
//! Los precios son inventados: no hay datos de mercado en el repo.

use std::num::NonZeroU32;

use cfds_core::{
    Candle, DailyCandle, Direction, DistMetric, FvgEvent, FvgEventPoint, FvgReaction,
    LiquidityStats, RateMetric, SizeBucket, StatsSubject, StatsTimeframe, Timeframe, ZoneStatus,
};
use cfds_liquidity::{StatsConfig, fvg_events, fvg_stats, gap_stats, opening_gaps};
use chrono::{DateTime, Datelike, NaiveDate, TimeDelta, Utc, Weekday};
use rust_decimal::{Decimal, dec};

const ATR: usize = 14;

fn cfg() -> StatsConfig {
    StatsConfig::from_toml(include_str!("../../../../config/liquidity.toml")).unwrap()
}

fn nz(n: u32) -> NonZeroU32 {
    NonZeroU32::new(n).unwrap()
}

fn t0() -> DateTime<Utc> {
    "2026-11-16T14:00:00Z".parse().unwrap()
}

/// Vela de 15m en la posición `slot` (las posiciones saltadas son velas faltantes).
fn bar(slot: i32, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> Candle {
    Candle {
        instrument_id: "US100".parse().unwrap(),
        timeframe: Timeframe::M15,
        start: t0() + TimeDelta::minutes(15) * slot,
        open: o,
        high: h,
        low: l,
        close: c,
        volume: None,
    }
}

/// Vela chata de rango 2 alrededor de `level`.
fn flat(slot: i32, level: Decimal) -> Candle {
    bar(slot, level, level + dec!(1), level - dec!(1), level)
}

/// 15 velas chatas (ATR14 válido) y un FVG alcista en la vela 17: zona [101, 103], mitad 102.
fn with_bullish_fvg() -> Vec<Candle> {
    let mut v: Vec<Candle> = (0..16).map(|i| flat(i, dec!(100))).collect();
    v.push(bar(16, dec!(100), dec!(106), dec!(100), dec!(105)));
    v.push(bar(17, dec!(105), dec!(108), dec!(103), dec!(107)));
    v
}

/// Después del FVG: toca (L 102,5), llega a la mitad (L 101,5) y lo llena (C 100,5 < 101).
fn touch_mid_fill(first_slot: i32) -> Vec<Candle> {
    let s = first_slot;
    vec![
        bar(s, dec!(107), dec!(108), dec!(102.5), dec!(104)),
        bar(s + 1, dec!(104), dec!(105), dec!(101.5), dec!(103)),
        bar(s + 2, dec!(103), dec!(103), dec!(100), dec!(100.5)),
    ]
}

fn only(events: Vec<FvgEvent>) -> FvgEvent {
    assert_eq!(events.len(), 1, "{events:#?}");
    events.into_iter().next().unwrap()
}

#[test]
fn records_touch_mid_and_fill_with_bars_since_creation() {
    let mut v = with_bullish_fvg();
    v.extend(touch_mid_fill(18));
    let e = only(fvg_events(&v, ATR, &cfg()));
    assert_eq!(e.direction, Direction::Bullish);
    assert_eq!(e.timeframe, StatsTimeframe::M15);
    assert_eq!((e.bottom, e.top), (dec!(101), dec!(103)));
    assert_eq!(e.created_at, v[17].end());
    assert_eq!(e.size_atr, (e.top - e.bottom) / e.atr);
    let bars = |p: Option<FvgEventPoint>| p.map(|p| p.bars.get());
    assert_eq!(
        (bars(e.touch), bars(e.mid), bars(e.fill)),
        (Some(1), Some(2), Some(3))
    );
    assert_eq!(e.fill.unwrap().at, v[20].end());
    assert_eq!(e.status, ZoneStatus::Filled);
    assert_eq!(e.expiry, None);
    assert_eq!(e.as_of, v[20].end());
}

#[test]
fn no_look_ahead_an_open_fvg_counts_as_touched_but_not_filled_or_expired() {
    let mut v = with_bullish_fvg();
    // Antes del cierre de la vela 17 el FVG no existe.
    assert!(fvg_events(&v[..17], ATR, &cfg()).is_empty());
    v.extend(touch_mid_fill(18));
    let e = only(fvg_events(&v[..19], ATR, &cfg()));
    assert_eq!(e.touch.map(|p| p.bars.get()), Some(1));
    assert_eq!((e.mid, e.fill, e.expiry), (None, None, None));
    assert_eq!(e.status, ZoneStatus::Open);
    assert!(e.reactions.iter().all(|r| r.return_atr.is_none()));

    let mut small = cfg();
    small.min_cases = nz(1);
    let s = all_sizes(&fvg_stats(&[e], &small), Direction::Bullish);
    assert_eq!(rate(&s, RateMetric::Touch), (1, 1, Some(dec!(1))));
    // Un FVG abierto no entra al denominador del llenado.
    assert_eq!(rate(&s, RateMetric::Fill).1, 0);
}

#[test]
fn expires_after_max_age_bars_without_fill() {
    let mut v = with_bullish_fvg();
    v.extend((18..22).map(|i| flat(i, dec!(105))));
    let mut c = cfg();
    c.max_age_bars = nz(3);
    let e = only(fvg_events(&v, ATR, &c));
    assert_eq!(e.status, ZoneStatus::Expired);
    assert_eq!(
        e.expiry.map(|p| (p.bars.get(), p.at)),
        Some((3, v[20].end()))
    );
    // Con una vela menos todavía está abierto: no vence antes de tiempo.
    assert_eq!(only(fvg_events(&v[..20], ATR, &c)).status, ZoneStatus::Open);
}

#[test]
fn reaction_is_signed_in_favour_of_the_direction() {
    // FVG bajista: el espejo del alcista, zona [97, 99].
    let mut v: Vec<Candle> = (0..16).map(|i| flat(i, dec!(100))).collect();
    v.push(bar(16, dec!(100), dec!(100), dec!(94), dec!(95)));
    v.push(bar(17, dec!(95), dec!(97), dec!(92), dec!(93)));
    v.push(bar(18, dec!(93), dec!(97.5), dec!(92), dec!(96))); // toque, cierre 96
    v.extend((19..30).map(|i| flat(i, dec!(93)))); // el precio baja: a favor del FVG bajista
    let e = only(fvg_events(&v, ATR, &cfg()));
    assert_eq!(e.direction, Direction::Bearish);
    assert_eq!((e.bottom, e.top), (dec!(97), dec!(99)));
    assert_eq!(e.touch.map(|p| p.bars.get()), Some(1));
    assert_eq!(
        e.reactions,
        vec![
            FvgReaction {
                bars: nz(4),
                return_atr: Some(dec!(3) / e.atr),
                at: Some(v[22].end()),
            },
            FvgReaction {
                bars: nz(8),
                return_atr: Some(dec!(3) / e.atr),
                at: Some(v[26].end()),
            },
        ]
    );
}

#[test]
fn a_zero_size_gap_is_not_an_fvg() {
    let mut v: Vec<Candle> = (0..16).map(|i| flat(i, dec!(100))).collect();
    v.push(bar(16, dec!(100), dec!(106), dec!(100), dec!(105)));
    v.push(bar(17, dec!(105), dec!(108), dec!(101), dec!(107))); // L = H de la vela 15
    assert!(fvg_events(&v, ATR, &cfg()).is_empty());
}

#[test]
fn no_fvg_before_the_atr_warms_up() {
    let mut v: Vec<Candle> = (0..2).map(|i| flat(i, dec!(100))).collect();
    v.push(bar(2, dec!(100), dec!(106), dec!(100), dec!(105)));
    v.push(bar(3, dec!(105), dec!(108), dec!(103), dec!(107)));
    assert!(fvg_events(&v, ATR, &cfg()).is_empty());
}

#[test]
fn missing_candles_are_not_filled_bars_count_present_candles() {
    let mut v = with_bullish_fvg();
    v.extend(touch_mid_fill(18));
    // Faltan dos velas entre el toque y la mitad: las posteriores se corren dos lugares.
    let mut gapped = with_bullish_fvg();
    let mut after = touch_mid_fill(18);
    for c in after.iter_mut().skip(1) {
        c.start += TimeDelta::minutes(30);
    }
    gapped.extend(after);
    let (a, b) = (
        only(fvg_events(&v, ATR, &cfg())),
        only(fvg_events(&gapped, ATR, &cfg())),
    );
    assert_eq!(b.mid.unwrap().bars, a.mid.unwrap().bars);
    assert_eq!(b.mid.unwrap().at, gapped[19].end());
    assert_eq!(
        b.mid.unwrap().at - a.mid.unwrap().at,
        TimeDelta::minutes(30)
    );
}

/// Serie pseudoaleatoria determinista (LCG) con muchos FVG de los dos lados.
fn noisy(n: i32) -> Vec<Candle> {
    let mut seed: u64 = 42;
    let mut next = || {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        i64::try_from(seed >> 59).unwrap() - 16 // −16..=15
    };
    let mut close = dec!(1000);
    (0..n)
        .map(|i| {
            let open = close;
            close = open + Decimal::from(next());
            let high = open.max(close) + Decimal::from(next().abs() / 4);
            let low = open.min(close) - Decimal::from(next().abs() / 4);
            bar(i, open, high, low, close)
        })
        .collect()
}

#[test]
fn no_repaint_new_candles_never_change_a_past_event() {
    let v = noisy(400);
    let mut c = cfg();
    c.max_age_bars = nz(40);
    let full = fvg_events(&v, ATR, &c);
    assert!(full.len() > 20, "la serie de prueba tiene pocos FVG");
    assert!(full.iter().any(|e| e.status == ZoneStatus::Filled));
    assert!(full.iter().any(|e| e.status == ZoneStatus::Expired));
    for k in (20..400).step_by(7) {
        let prefix = fvg_events(&v[..k], ATR, &c);
        let cutoff = v[k - 1].end();
        // Los FVG ya creados son los mismos: ni aparecen ni desaparecen después.
        let created: Vec<_> = full.iter().filter(|e| e.created_at <= cutoff).collect();
        assert_eq!(prefix.len(), created.len());
        for (p, f) in prefix.iter().zip(created) {
            assert_eq!(
                (p.created_at, p.direction, p.bottom, p.top, p.atr),
                (f.created_at, f.direction, f.bottom, f.top, f.atr)
            );
            for (a, b) in [
                (p.touch, f.touch),
                (p.mid, f.mid),
                (p.fill, f.fill),
                (p.expiry, f.expiry),
            ] {
                assert!(a.is_none() || a == b, "repintado: {p:?} vs {f:?}");
            }
            for (a, b) in p.reactions.iter().zip(&f.reactions) {
                assert!(a.return_atr.is_none() || a == b);
            }
            if p.status != ZoneStatus::Open {
                assert_eq!(p.status, f.status);
            }
        }
    }
}

// ---------- gaps de apertura (diario) ----------

/// Velas diarias de días hábiles desde el lunes 5-1-2026 (sin sábados ni domingos).
fn sessions(ohlc: &[(Decimal, Decimal, Decimal, Decimal)]) -> Vec<DailyCandle> {
    let mut day = NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
    ohlc.iter()
        .map(|&(open, high, low, close)| {
            while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
                day = day.succ_opt().unwrap();
            }
            let start = day.and_hms_opt(5, 0, 0).unwrap().and_utc();
            let c = DailyCandle {
                instrument_id: "US500".parse().unwrap(),
                day,
                start,
                end: start + TimeDelta::days(1),
                open,
                high,
                low,
                close,
                volume: None,
            };
            day = day.succ_opt().unwrap();
            c
        })
        .collect()
}

fn flat_day() -> (Decimal, Decimal, Decimal, Decimal) {
    (dec!(100), dec!(101), dec!(99), dec!(100))
}

#[test]
fn weekend_gap_is_measured_against_friday_and_filled_later() {
    // 15 sesiones chatas (ATR 2): la última es un viernes. El lunes abre en 103 (≥ 0,25 × 2).
    let mut ohlc = vec![flat_day(); 15];
    ohlc.push((dec!(103), dec!(104), dec!(101), dec!(103.5))); // lunes: no llega a 100
    ohlc.push((dec!(103), dec!(103), dec!(99.5), dec!(100))); // martes: toca 100
    let d = sessions(&ohlc);
    assert_eq!(d[14].day.weekday(), Weekday::Fri);
    assert_eq!(d[15].day.weekday(), Weekday::Mon);
    let gaps = opening_gaps(&d, ATR, &cfg());
    assert_eq!(gaps.len(), 1);
    let g = &gaps[0];
    assert_eq!(
        (g.day, g.direction, g.prev_close, g.open),
        (d[15].day, Direction::Bullish, dec!(100), dec!(103))
    );
    assert_eq!(g.size_atr, dec!(3) / g.atr);
    assert_eq!(
        g.fill.map(|f| (f.day, f.sessions, f.at)),
        Some((d[16].day, 1, Some(d[16].end)))
    );
    assert!(!g.same_session);
    assert_eq!(g.status, ZoneStatus::Filled);
    // Al cierre del lunes el gap está abierto (sin look-ahead).
    let open = &opening_gaps(&d[..16], ATR, &cfg())[0];
    assert_eq!(
        (open.fill, open.status, open.as_of),
        (None, ZoneStatus::Open, d[15].end)
    );
}

#[test]
fn same_session_fill_small_gaps_and_expiry() {
    let mut ohlc = vec![flat_day(); 15];
    ohlc.push((dec!(97), dec!(100.2), dec!(96), dec!(99))); // gap bajista, se llena en la sesión
    ohlc.push((dec!(99.2), dec!(99.5), dec!(98.5), dec!(99))); // 0,2 < 0,25 × ATR: no es gap
    ohlc.push((dec!(95), dec!(96), dec!(94), dec!(95))); // gap bajista que no se llena
    ohlc.extend(vec![(dec!(95), dec!(96), dec!(94), dec!(95)); 3]);
    let d = sessions(&ohlc);
    let mut c = cfg();
    c.gap_max_sessions = nz(2);
    let gaps = opening_gaps(&d, ATR, &c);
    assert_eq!(gaps.len(), 2, "{gaps:#?}");
    assert_eq!(
        (gaps[0].direction, gaps[0].same_session),
        (Direction::Bearish, true)
    );
    assert_eq!(gaps[0].fill.map(|f| f.sessions), Some(0));
    assert_eq!(gaps[1].status, ZoneStatus::Expired);
    assert_eq!(
        gaps[1].expiry.map(|e| (e.day, e.sessions)),
        Some((d[19].day, 2))
    );

    c.min_cases = nz(1);
    let s = all_sizes(&gap_stats(&gaps, &c), Direction::Bearish);
    assert_eq!(s.subject, StatsSubject::OpeningGap);
    assert_eq!(s.timeframe, StatsTimeframe::D1);
    assert_eq!(rate(&s, RateMetric::Fill), (1, 2, Some(dec!(0.5))));
    assert_eq!(
        rate(&s, RateMetric::SameSessionFill),
        (1, 2, Some(dec!(0.5)))
    );
}

// ---------- agregados ----------

fn event(i: i64, touched: bool, size_atr: Decimal) -> FvgEvent {
    let at = t0() + TimeDelta::minutes(15 * i);
    let point = |bars| {
        Some(FvgEventPoint {
            at: at + TimeDelta::minutes(15 * i64::from(bars)),
            bars: nz(bars),
        })
    };
    FvgEvent {
        instrument_id: "US100".parse().unwrap(),
        timeframe: StatsTimeframe::M15,
        direction: Direction::Bullish,
        created_at: at,
        bottom: dec!(100),
        top: dec!(100) + size_atr,
        atr: dec!(1),
        size_atr,
        touch: if touched { point(2) } else { None },
        mid: None,
        fill: None,
        expiry: point(500),
        reactions: vec![FvgReaction {
            bars: nz(4),
            return_atr: touched.then_some(dec!(0.5)),
            at: None,
        }],
        status: ZoneStatus::Expired,
        as_of: at + TimeDelta::days(30),
    }
}

fn all_sizes(stats: &[LiquidityStats], dir: Direction) -> LiquidityStats {
    stats
        .iter()
        .find(|s| s.direction == dir && s.size_tercile == SizeBucket::All)
        .cloned()
        .unwrap()
}

fn rate(s: &LiquidityStats, m: RateMetric) -> (u32, u32, Option<Decimal>) {
    let r = s.rates.iter().find(|r| r.metric == m).unwrap();
    (r.successes, r.n, r.rate)
}

#[test]
fn wilson_interval_of_the_aggregates_matches_hand_computed_values() {
    // 7 de 20 tocados (con min_cases = 20): p = 0,35, IC [0,1812; 0,5671].
    let mut c = cfg();
    c.min_cases = nz(20);
    let events: Vec<_> = (0..20).map(|i| event(i, i < 7, dec!(1))).collect();
    let s = all_sizes(&fvg_stats(&events, &c), Direction::Bullish);
    let r = s
        .rates
        .iter()
        .find(|r| r.metric == RateMetric::Touch)
        .unwrap();
    assert_eq!(
        (r.rate, r.ci_low, r.ci_high),
        (Some(dec!(0.35)), Some(dec!(0.1812)), Some(dec!(0.5671)))
    );
    // 0 de 30: IC [0; 0,1135].
    let events: Vec<_> = (0..30).map(|i| event(i, false, dec!(1))).collect();
    let s = all_sizes(&fvg_stats(&events, &cfg()), Direction::Bullish);
    let r = s
        .rates
        .iter()
        .find(|r| r.metric == RateMetric::Touch)
        .unwrap();
    assert_eq!(
        (r.rate, r.ci_low, r.ci_high),
        (Some(dec!(0)), Some(dec!(0)), Some(dec!(0.1135)))
    );
}

#[test]
fn fewer_cases_than_min_cases_is_insufficient_data() {
    let events: Vec<_> = (0..29).map(|i| event(i, true, dec!(1))).collect();
    let s = all_sizes(&fvg_stats(&events, &cfg()), Direction::Bullish);
    assert_eq!(s.n, 29);
    for r in &s.rates {
        assert!(r.insufficient && r.rate.is_none() && r.ci_low.is_none() && r.ci_high.is_none());
    }
    for d in &s.distributions {
        assert!(d.insufficient && d.median.is_none() && d.iqr.is_none());
    }
}

#[test]
fn groups_by_size_tercile_with_medians_and_iqr() {
    // 90 FVG de tamaños 1..=90 (en ATR): cortes en 30,67 y 60,33; 30 casos por tercil.
    let events: Vec<_> = (1..=90).map(|i| event(i, true, Decimal::from(i))).collect();
    let stats = fvg_stats(&events, &cfg());
    assert_eq!(stats.len(), 4); // solo alcistas: low, mid, high y all
    let low = stats
        .iter()
        .find(|s| s.size_tercile == SizeBucket::Low)
        .unwrap();
    assert_eq!(low.n, 30);
    assert_eq!(low.size_cuts_atr.low_max.round_dp(10), dec!(30.6666666667));
    assert_eq!(
        stats
            .iter()
            .find(|s| s.size_tercile == SizeBucket::All)
            .unwrap()
            .n,
        90
    );
    let touch = low
        .distributions
        .iter()
        .find(|d| d.metric == DistMetric::BarsToTouch)
        .unwrap();
    assert_eq!((touch.median, touch.iqr), (Some(dec!(2)), Some(dec!(0))));
    let reaction = low
        .distributions
        .iter()
        .find(|d| d.metric == DistMetric::Reaction && d.horizon_bars == Some(nz(4)))
        .unwrap();
    assert_eq!((reaction.n, reaction.median), (30, Some(dec!(0.5))));
}

#[test]
fn daily_fvgs_use_the_same_definition() {
    let mut ohlc = vec![flat_day(); 16];
    ohlc.push((dec!(100), dec!(106), dec!(100), dec!(105)));
    ohlc.push((dec!(105), dec!(108), dec!(103), dec!(107)));
    let d = sessions(&ohlc);
    let e = only(fvg_events(&d, ATR, &cfg()));
    assert_eq!(
        (e.timeframe, e.direction),
        (StatsTimeframe::D1, Direction::Bullish)
    );
    assert_eq!(
        (e.bottom, e.top, e.created_at),
        (dec!(101), dec!(103), d[17].end)
    );
}
