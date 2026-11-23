//! Barras de 1 minuto de futuros (#248): agrupación sin mezclar contratos y verificación de las
//! diarias contra IG, con series sintéticas construidas a mano.

use cfds_core::{DailyCandle, FuturesBar, InstrumentId, Timeframe};
use cfds_market_data::{aggregate_bars, compare_daily};
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
fn fifteen_minute_candles_come_only_from_closed_buckets_of_one_contract() {
    let mut bars = minutes("2026-11-18T09:00:00Z", 15, 1, dec!(100)); // 09:00 completa
    bars.extend(minutes("2026-11-18T09:15:00Z", 10, 1, dec!(200))); // 09:15 con 10 barras
    bars.extend(minutes("2026-11-18T09:25:00Z", 5, 2, dec!(300))); // vencimiento en el medio
    bars.extend(minutes("2026-11-18T09:30:00Z", 3, 2, dec!(400))); // 09:30 todavía abierta
    let a = aggregate_bars(&bars, &de40(), Timeframe::M15, at("2026-11-18T09:40:00Z"));
    assert_eq!(a.mixed_contracts, 1);
    assert_eq!(a.candles.len(), 1);
    let c = &a.candles[0];
    assert_eq!(
        (c.start, c.open, c.high, c.low, c.close, c.volume),
        (
            at("2026-11-18T09:00:00Z"),
            dec!(99),
            dec!(116),
            dec!(97),
            dec!(114),
            Some(dec!(150))
        )
    );
    // En 1h, la vela de las 09:00 tiene dos contratos: no se arma.
    let h = aggregate_bars(&bars, &de40(), Timeframe::H1, at("2026-11-18T10:00:00Z"));
    assert_eq!((h.candles.len(), h.mixed_contracts), (0, 1));
    // Sin repintado: agregar barras no cambia las velas ya cerradas.
    let fewer = aggregate_bars(
        &bars[..20],
        &de40(),
        Timeframe::M15,
        at("2026-11-18T09:40:00Z"),
    );
    assert_eq!(fewer.candles[0], a.candles[0]);
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
