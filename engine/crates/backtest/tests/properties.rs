//! Tests de propiedades de los costos y del P/L con decimales exactos (#663, plan B1 de
//! `docs/analysis/2026-11-autoanalisis.md`).
//!
//! - La ejecución siempre es en contra: comprar nunca sale más barato que el medio, vender nunca
//!   más caro.
//! - Una ida y vuelta sin que el precio se mueva siempre pierde exactamente
//!   `tamaño × (spread + 2 × slippage)`: los costos nunca son cero (CLAUDE.md §5b).
//! - Compra y venta son espejo: el P/L de un largo con el medio subiendo `d` es el de un corto
//!   con el medio bajando `d`.
//! - La comisión nunca baja del mínimo; las noches de financiación se suman sin contar dos
//!   veces el mismo corte.
//!
//! El P/L bruto se arma como en el motor: `tamaño × (salida − entrada) × lado`.

use cfds_backtest::{
    Commission, CostConfig, Execution, InstrumentCosts, commission, fill_price, funding,
    funding_nights,
};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, TimeDelta, Utc};
use proptest::prelude::*;
use rust_decimal::Decimal;

const CASES: u32 = 256;

fn costs() -> impl Strategy<Value = (InstrumentCosts, CostConfig)> {
    (0i64..1_000, 1i64..100, 0i64..50, 0i64..5, 0i64..500).prop_map(
        |(spread, tick, frac, ticks, fee)| {
            (
                InstrumentCosts {
                    spread: Decimal::new(spread, 2),
                    tick: Decimal::new(tick, 2),
                    commission: Commission::None,
                    funding_admin_fee_pct: Decimal::new(fee, 2),
                    funding_day_count: Decimal::from(365),
                },
                CostConfig {
                    slippage_spread_fraction: Decimal::new(frac, 2),
                    slippage_ticks: Decimal::from(ticks),
                },
            )
        },
    )
}

fn side() -> impl Strategy<Value = Side> {
    prop_oneof![Just(Side::Long), Just(Side::Short)]
}

fn sign(side: Side) -> Decimal {
    match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    }
}

/// P/L bruto de una operación entre dos precios medios, con la ejecución en contra.
fn gross(
    side: Side,
    size: Decimal,
    entry_mid: Decimal,
    exit_mid: Decimal,
    c: &(InstrumentCosts, CostConfig),
) -> Decimal {
    let entry = fill_price(side, Execution::Entry, entry_mid, &c.0, &c.1);
    let exit = fill_price(side, Execution::Exit, exit_mid, &c.0, &c.1);
    size * (exit - entry) * sign(side)
}

fn price() -> impl Strategy<Value = Decimal> {
    (100_000i64..5_000_000).prop_map(|p| Decimal::new(p, 2))
}

fn size() -> impl Strategy<Value = Decimal> {
    (1i64..1_000_000).prop_map(|s| Decimal::new(s, 4))
}

fn utc(minutes: i64) -> DateTime<Utc> {
    "2026-01-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap() + TimeDelta::minutes(minutes)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    #[test]
    fn execution_is_always_adverse(c in costs(), mid in price(), side in side()) {
        let entry = fill_price(side, Execution::Entry, mid, &c.0, &c.1);
        let exit = fill_price(side, Execution::Exit, mid, &c.0, &c.1);
        match side {
            Side::Long => prop_assert!(entry >= mid && exit <= mid),
            Side::Short => prop_assert!(entry <= mid && exit >= mid),
        }
    }

    #[test]
    fn a_flat_round_trip_loses_exactly_the_costs(
        c in costs(),
        mid in price(),
        size in size(),
        side in side(),
    ) {
        let slippage = c.1.slippage_spread_fraction * c.0.spread + c.1.slippage_ticks * c.0.tick;
        let expected = -(size * (c.0.spread + Decimal::TWO * slippage));
        prop_assert_eq!(gross(side, size, mid, mid, &c), expected);
        prop_assert!(expected <= Decimal::ZERO);
    }

    #[test]
    fn long_and_short_are_mirrors(
        c in costs(),
        mid in price(),
        size in size(),
        move_cents in -100_000i64..100_000,
    ) {
        let d = Decimal::new(move_cents, 2);
        prop_assert_eq!(
            gross(Side::Long, size, mid, mid + d, &c),
            gross(Side::Short, size, mid, mid - d, &c)
        );
    }

    #[test]
    fn a_better_exit_never_lowers_the_pnl(
        c in costs(),
        mid in price(),
        size in size(),
        side in side(),
        a in -100_000i64..100_000,
        b in 0i64..100_000,
    ) {
        // `b` más a favor del lado: el P/L no puede bajar.
        let worse = mid + Decimal::new(a, 2) * sign(side);
        let better = worse + Decimal::new(b, 2) * sign(side);
        prop_assert!(gross(side, size, mid, better, &c) >= gross(side, size, mid, worse, &c));
    }

    #[test]
    fn commission_never_goes_below_the_minimum(
        per_share in 0i64..10_000,
        pct in 0i64..100,
        min in 0i64..10_000,
        size in size(),
        price in price(),
    ) {
        let min = Decimal::new(min, 2);
        for kind in [
            Commission::PerShare { per_share: Decimal::new(per_share, 4), min },
            Commission::Percent { pct: Decimal::new(pct, 2), min },
        ] {
            let c = InstrumentCosts {
                spread: Decimal::ZERO,
                tick: Decimal::ONE,
                commission: kind,
                funding_admin_fee_pct: Decimal::ZERO,
                funding_day_count: Decimal::from(365),
            };
            prop_assert!(commission(&c, size, price) >= min);
        }
    }

    #[test]
    fn funding_is_zero_without_nights_and_never_favours_the_long_when_rates_are_positive(
        c in costs(),
        close in price(),
        size in size(),
        nights in 0u32..30,
        benchmark in 0i64..800,
    ) {
        let benchmark = Decimal::new(benchmark, 2);
        prop_assert_eq!(funding(Side::Long, close, size, 0, benchmark, &c.0), Decimal::ZERO);
        let long = funding(Side::Long, close, size, nights, benchmark, &c.0);
        let short = funding(Side::Short, close, size, nights, benchmark, &c.0);
        prop_assert!(long >= Decimal::ZERO);
        prop_assert!(long >= short);
    }

    #[test]
    fn funding_nights_add_up_over_consecutive_periods(
        a in 0i64..200_000,
        ab in 0i64..20_000,
        bc in 0i64..20_000,
    ) {
        let cutoff = NaiveTime::from_hms_opt(23, 0, 0).unwrap();
        let tz = chrono_tz::Europe::Madrid;
        let (a, b, c) = (utc(a), utc(a + ab), utc(a + ab + bc));
        prop_assert_eq!(
            funding_nights(a, c, cutoff, tz),
            funding_nights(a, b, cutoff, tz) + funding_nights(b, c, cutoff, tz)
        );
    }
}
