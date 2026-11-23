//! Barras de 1 minuto de futuros (#248): agrupación sin mezclar contratos y verificación de las
//! diarias contra IG, con series sintéticas construidas a mano.

use cfds_core::{DailyCandle, FuturesBar, InstrumentId, Timeframe};
use cfds_market_data::{compare_daily, futures_candles};
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::{Decimal, dec};

fn at(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn de40() -> InstrumentId {
    "DE40".parse().unwrap()
}

fn bar(start: DateTime<Utc>, contract: u64, close: Decimal) -> FuturesBar {
    FuturesBar {
        source: "databento".into(),
        dataset: "XEUR.EOBI".into(),
        symbol: "FDAX.c.0".into(),
        instrument_id: de40(),
        contract_id: contract,
        start,
        open: close - dec!(1),
        high: close + dec!(2),
        low: close - dec!(3),
        close,
        volume: 10,
    }
}

/// `n` barras seguidas desde `from`, con cierre `base + i`.
fn minutes(from: &str, n: i64, contract: u64, base: Decimal) -> Vec<FuturesBar> {
    (0..n)
        .map(|i| {
            bar(
                at(from) + TimeDelta::minutes(i),
                contract,
                base + Decimal::from(i),
            )
        })
        .collect()
}

#[test]
fn fifteen_minute_candles_come_only_from_closed_windows_of_one_contract() {
    let mut bars = minutes("2026-11-18T09:00:00Z", 15, 1, dec!(100)); // 09:00 completa
    bars.extend(minutes("2026-11-18T09:15:00Z", 10, 1, dec!(200))); // 09:15 con 10 barras
    bars.extend(minutes("2026-11-18T09:25:00Z", 5, 2, dec!(300))); // vencimiento en el medio
    bars.extend(minutes("2026-11-18T09:30:00Z", 3, 2, dec!(400))); // 09:30 todavía abierta
    let f = futures_candles(&bars, &de40(), at("2026-11-18T09:40:00Z")).unwrap();
    assert_eq!(f.m15.len(), 1);
    let c = &f.m15[0];
    assert_eq!(
        (
            c.candle.start,
            c.candle.open,
            c.candle.high,
            c.candle.low,
            c.candle.close
        ),
        (
            at("2026-11-18T09:00:00Z"),
            dec!(99),
            dec!(116),
            dec!(97),
            dec!(114)
        )
    );
    assert_eq!((c.candle.volume, c.contract_id), (Some(dec!(150)), 1));
    assert_eq!(c.candle.timeframe, Timeframe::M15);
    // El vencimiento dentro de 09:15 descarta esa vela y la hora de las 09:00, y se avisa.
    assert_eq!(
        f.mixed,
        vec![
            (Timeframe::M15, at("2026-11-18T09:15:00Z")),
            (Timeframe::H1, at("2026-11-18T09:00:00Z"))
        ]
    );
    assert!(f.h1.is_empty());
}

#[test]
fn no_repaint_adding_bars_never_changes_closed_candles() {
    let mut bars = Vec::new();
    for h in 0..6 {
        bars.extend(minutes(
            &format!("2026-11-18T{:02}:00:00Z", 8 + h),
            60,
            7,
            Decimal::from(100 * (h + 1)),
        ));
    }
    let end = at("2026-11-18T14:00:00Z");
    let full = futures_candles(&bars, &de40(), end).unwrap();
    assert_eq!((full.m15.len(), full.h1.len()), (24, 6));
    for n in [1, 14, 15, 16, 59, 60, 61, 200, 359] {
        // Con datos hasta la barra n (exclusive), lo bajado llega hasta su inicio.
        let until = bars[n].start;
        let part = futures_candles(&bars[..n], &de40(), until).unwrap();
        assert_eq!(part.m15[..], full.m15[..part.m15.len()], "con {n} barras");
        assert_eq!(part.h1[..], full.h1[..part.h1.len()], "con {n} barras");
        // Ninguna vela termina después de lo bajado.
        assert!(part.m15.iter().all(|c| c.candle.end() <= until));
        assert!(part.h1.iter().all(|c| c.candle.end() <= until));
    }
}

#[test]
fn gaps_are_not_filled_and_an_hour_needs_its_four_quarters() {
    let mut bars = minutes("2026-11-18T09:00:00Z", 15, 7, dec!(100));
    // 09:15: solo dos minutos con operaciones; 09:30: ninguno; 09:45: completa.
    bars.push(bar(at("2026-11-18T09:17:00Z"), 7, dec!(120)));
    bars.push(bar(at("2026-11-18T09:28:00Z"), 7, dec!(121)));
    bars.extend(minutes("2026-11-18T09:45:00Z", 15, 7, dec!(130)));
    let f = futures_candles(&bars, &de40(), at("2026-11-18T10:00:00Z")).unwrap();
    let starts: Vec<_> = f.m15.iter().map(|c| c.candle.start).collect();
    assert_eq!(
        starts,
        vec![
            at("2026-11-18T09:00:00Z"),
            at("2026-11-18T09:15:00Z"),
            at("2026-11-18T09:45:00Z")
        ]
    );
    // La de 09:15 sale de sus dos barras: no se inventan los minutos sin operaciones.
    assert_eq!(
        (f.m15[1].candle.open, f.m15[1].candle.close),
        (dec!(119), dec!(121))
    );
    // Sin la vela de 09:30, la hora de las 09:00 no se arma (spec §3, #150). No es un vencimiento.
    assert!(f.h1.is_empty());
    assert!(f.mixed.is_empty());
}

#[test]
fn windows_are_aligned_in_utc_like_ig() {
    // Barras desde las 13:07: la primera vela es la de 13:00, con lo que haya.
    let bars = minutes("2026-11-18T13:07:00Z", 113, 7, dec!(100)); // hasta 14:59
    let f = futures_candles(&bars, &de40(), at("2026-11-18T15:00:00Z")).unwrap();
    assert!(f.m15.iter().all(|c| c.candle.start.timestamp() % 900 == 0));
    assert_eq!(f.m15[0].candle.start, at("2026-11-18T13:00:00Z"));
    // 13:00 tiene sus cuatro de 15m (13:00 con barras desde 13:07); 14:00 también.
    let hours: Vec<_> = f.h1.iter().map(|c| c.candle.start).collect();
    assert_eq!(
        hours,
        vec![at("2026-11-18T13:00:00Z"), at("2026-11-18T14:00:00Z")]
    );
    assert_eq!(f.h1[0].candle.timeframe, Timeframe::H1);
    assert_eq!(f.h1[0].candle.open, dec!(99));
}

#[test]
fn a_rollover_between_quarters_drops_the_hour_but_keeps_the_quarters() {
    let mut bars = minutes("2026-11-18T09:00:00Z", 30, 7, dec!(100));
    bars.extend(minutes("2026-11-18T09:30:00Z", 30, 8, dec!(250)));
    let f = futures_candles(&bars, &de40(), at("2026-11-18T10:00:00Z")).unwrap();
    let contracts: Vec<_> = f.m15.iter().map(|c| c.contract_id).collect();
    assert_eq!(contracts, vec![7, 7, 8, 8]);
    assert!(f.h1.is_empty());
    assert_eq!(f.mixed, vec![(Timeframe::H1, at("2026-11-18T09:00:00Z"))]);
}

fn ig_day(day: &str, close: Decimal) -> DailyCandle {
    // Diarias de IG de medianoche a medianoche en Europe/Berlin (CET en noviembre: UTC+1).
    let d: NaiveDate = day.parse().unwrap();
    let start = d.and_hms_opt(23, 0, 0).unwrap().and_utc() - TimeDelta::days(1);
    DailyCandle {
        instrument_id: de40(),
        day: d,
        start,
        end: start + TimeDelta::days(1),
        open: close,
        high: close + dec!(10),
        low: close - dec!(10),
        close,
        volume: None,
    }
}

fn session(day: &str, contract: u64, close: Decimal) -> Vec<FuturesBar> {
    // Una barra a las 08:00 y la última a las 20:59 UTC (el cierre del día).
    vec![
        bar(at(&format!("{day}T08:00:00Z")), contract, close - dec!(5)),
        bar(at(&format!("{day}T20:59:00Z")), contract, close),
    ]
}

#[test]
fn daily_comparison_measures_basis_and_correlates_returns_within_a_contract() {
    let ig = [
        ig_day("2026-11-16", dec!(24000)),
        ig_day("2026-11-17", dec!(24240)),
        ig_day("2026-11-18", dec!(24000)),
        ig_day("2026-11-19", dec!(24480)),
        ig_day("2026-11-20", dec!(24600)), // sin futuro ese día
    ];
    // El futuro cotiza 100 por encima (base −100) y se mueve igual.
    let mut bars = Vec::new();
    for (d, c) in [
        ("2026-11-16", dec!(24100)),
        ("2026-11-17", dec!(24340)),
        ("2026-11-18", dec!(24100)),
        ("2026-11-19", dec!(24580)),
    ] {
        bars.extend(session(d, 7, c));
    }
    let r = compare_daily(&ig, &bars);
    assert_eq!(r.missing, vec!["2026-11-20".parse::<NaiveDate>().unwrap()]);
    assert_eq!(r.days.len(), 4);
    assert!(r.days.iter().all(|d| d.basis == dec!(-100) && !d.rolled));
    assert_eq!(
        (r.basis_mean, r.basis_std_dev),
        (Some(dec!(-100)), Some(Decimal::ZERO))
    );
    assert_eq!(r.return_pairs, 3);
    let corr = r.correlation.unwrap();
    assert!(corr > dec!(0.999), "{corr}");
    assert_eq!(r.by_contract.len(), 1);
}

#[test]
fn a_rollover_day_does_not_count_as_a_return_and_basis_is_kept_per_contract() {
    let ig = [
        ig_day("2026-11-16", dec!(24000)),
        ig_day("2026-11-17", dec!(24100)),
        ig_day("2026-11-18", dec!(24200)),
        ig_day("2026-11-19", dec!(24300)),
    ];
    let mut bars = session("2026-11-16", 7, dec!(24100));
    // El 17 vence: la mañana es del contrato 7 y la tarde del 8, 150 más caro.
    bars.push(bar(at("2026-11-17T08:00:00Z"), 7, dec!(24190)));
    bars.push(bar(at("2026-11-17T20:59:00Z"), 8, dec!(24350)));
    bars.extend(session("2026-11-18", 8, dec!(24450)));
    bars.extend(session("2026-11-19", 8, dec!(24550)));
    let r = compare_daily(&ig, &bars);
    assert!(r.days[1].rolled);
    // El 17 tuvo vencimiento: el par 16→17 no cuenta; 17→18 y 18→19 sí (mismo contrato).
    assert_eq!(r.return_pairs, 2);
    let per: Vec<_> = r
        .by_contract
        .iter()
        .map(|c| (c.contract_id, c.days, c.mean))
        .collect();
    assert_eq!(per, vec![(7, 1, dec!(-100)), (8, 3, dec!(-250))]);
}

#[test]
fn bars_outside_the_ig_day_are_not_used() {
    let ig = [ig_day("2026-11-18", dec!(24000))];
    // 23:30 UTC del 18 ya es el 19 en Berlín: no cuenta para el 18.
    let bars = vec![
        bar(at("2026-11-18T20:59:00Z"), 7, dec!(24100)),
        bar(at("2026-11-18T23:30:00Z"), 7, dec!(25000)),
    ];
    let r = compare_daily(&ig, &bars);
    assert_eq!(r.days[0].futures_close, dec!(24100));
    assert_eq!(r.correlation, None);
}
