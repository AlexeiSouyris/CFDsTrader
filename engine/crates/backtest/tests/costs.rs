//! Costos calculables a mano (spec §11, #53).

use cfds_backtest::{
    BacktestConfig, Commission, CostConfig, Execution, InstrumentCosts, commission, fill_price,
    funding, funding_nights,
};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, Utc};
use rust_decimal::dec;

fn cfg() -> CostConfig {
    BacktestConfig::from_toml(include_str!("../../../../config/backtest.toml"))
        .unwrap()
        .costs
}

/// Índice con spread 1,6 y tick 0,1 (valores de ejemplo: los reales llegan con #8).
fn index() -> InstrumentCosts {
    InstrumentCosts {
        spread: dec!(1.6),
        tick: dec!(0.1),
        commission: Commission::None,
        funding_admin_fee_pct: dec!(2.5),
        funding_day_count: dec!(360),
    }
}

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

fn cutoff() -> NaiveTime {
    NaiveTime::from_hms_opt(23, 0, 0).unwrap()
}

// ---------- Ejecución ----------

#[test]
fn every_execution_pays_half_spread_plus_slippage_against_you() {
    // Medio spread 0,8 + slippage (0,1 × 1,6 + 1 × 0,1 = 0,26) = 1,06.
    let p = |side, ex| fill_price(side, ex, dec!(20000), &index(), &cfg());
    assert_eq!(p(Side::Long, Execution::Entry), dec!(20001.06));
    assert_eq!(p(Side::Long, Execution::Exit), dec!(19998.94));
    assert_eq!(p(Side::Short, Execution::Entry), dec!(19998.94));
    assert_eq!(p(Side::Short, Execution::Exit), dec!(20001.06));
}

#[test]
fn a_round_trip_at_the_same_mid_always_loses() {
    for side in [Side::Long, Side::Short] {
        let entry = fill_price(side, Execution::Entry, dec!(100), &index(), &cfg());
        let exit = fill_price(side, Execution::Exit, dec!(100), &index(), &cfg());
        let points = match side {
            Side::Long => exit - entry,
            Side::Short => entry - exit,
        };
        assert_eq!(points, dec!(-2.12), "{side:?}");
    }
}

#[test]
fn shares_without_spread_still_pay_slippage() {
    let share = InstrumentCosts {
        spread: dec!(0),
        tick: dec!(0.01),
        ..index()
    };
    assert_eq!(
        fill_price(Side::Long, Execution::Entry, dec!(150), &share, &cfg()),
        dec!(150.01)
    );
}

// ---------- Comisión ----------

#[test]
fn us_shares_pay_per_share_with_a_minimum() {
    let us = InstrumentCosts {
        commission: Commission::PerShare {
            per_share: dec!(0.02),
            min: dec!(15),
        },
        ..index()
    };
    // 100 acciones × 0,02 = 2 → mínimo 15.
    assert_eq!(commission(&us, dec!(100), dec!(150)), dec!(15));
    // 1.000 acciones × 0,02 = 20.
    assert_eq!(commission(&us, dec!(1000), dec!(150)), dec!(20));
}

#[test]
fn eu_shares_pay_a_percentage_with_a_minimum() {
    let eu = InstrumentCosts {
        commission: Commission::Percent {
            pct: dec!(0.06),
            min: dec!(5),
        },
        ..index()
    };
    // 10 × 40 = 400 de nocional × 0,06 % = 0,24 → mínimo 5.
    assert_eq!(commission(&eu, dec!(10), dec!(40)), dec!(5));
    // 1.000 × 40 = 40.000 × 0,06 % = 24.
    assert_eq!(commission(&eu, dec!(1000), dec!(40)), dec!(24));
}

#[test]
fn indices_pay_no_commission() {
    assert_eq!(commission(&index(), dec!(5), dec!(20000)), dec!(0));
}

// ---------- Financiación ----------

#[test]
fn long_pays_benchmark_plus_fee() {
    // 1 noche × 20.000 × 1 por punto × (3,5 + 2,5) % / 360 = 3,333…
    let f = funding(Side::Long, dec!(20000), dec!(1), 1, dec!(3.5), &index());
    assert_eq!(f.round_dp(4), dec!(3.3333));
    // 3 noches: el triple.
    let f3 = funding(Side::Long, dec!(20000), dec!(1), 3, dec!(3.5), &index());
    assert_eq!(f3, f * dec!(3));
}

#[test]
fn short_pays_the_fee_minus_the_benchmark_and_can_earn() {
    // (2,5 − 1,5) % → paga.
    let pays = funding(Side::Short, dec!(36000), dec!(1), 1, dec!(1.5), &index());
    assert_eq!(pays, dec!(1));
    // (2,5 − 4) % → cobra (negativo).
    let earns = funding(Side::Short, dec!(36000), dec!(1), 1, dec!(4), &index());
    assert_eq!(earns, dec!(-1.5));
}

#[test]
fn no_nights_no_funding() {
    assert_eq!(
        funding(Side::Long, dec!(20000), dec!(1), 0, dec!(3.5), &index()),
        dec!(0)
    );
}

// ---------- Noches ----------

#[test]
fn intraday_trades_pay_no_night() {
    // 15:30 a 21:45 de Madrid (invierno, UTC+1).
    let n = funding_nights(
        utc("2026-11-19T14:30:00Z"),
        utc("2026-11-19T20:45:00Z"),
        cutoff(),
        chrono_tz::Europe::Madrid,
    );
    assert_eq!(n, 0);
}

#[test]
fn the_cutoff_itself_counts_as_held() {
    // Cierra justo a las 23:00 de Madrid (22:00 UTC en invierno): estaba abierta en el corte.
    let tz = chrono_tz::Europe::Madrid;
    let open = utc("2026-11-19T14:30:00Z");
    assert_eq!(
        funding_nights(open, utc("2026-11-19T22:00:00Z"), cutoff(), tz),
        1
    );
    assert_eq!(
        funding_nights(open, utc("2026-11-19T21:59:59Z"), cutoff(), tz),
        0
    );
    // Abre justo en el corte: esa noche no la paga.
    assert_eq!(
        funding_nights(
            utc("2026-11-19T22:00:00Z"),
            utc("2026-11-20T10:00:00Z"),
            cutoff(),
            tz
        ),
        0
    );
}

#[test]
fn friday_to_monday_is_three_nights() {
    // Viernes 20-11-2026 15:00 Madrid → lunes 23-11-2026 10:00 Madrid.
    let n = funding_nights(
        utc("2026-11-20T14:00:00Z"),
        utc("2026-11-23T09:00:00Z"),
        cutoff(),
        chrono_tz::Europe::Madrid,
    );
    assert_eq!(n, 3);
}

#[test]
fn daylight_saving_changes_move_the_cutoff_in_utc() {
    let tz = chrono_tz::Europe::Madrid;
    // Verano (UTC+2): el corte de las 23:00 es a las 21:00 UTC.
    assert_eq!(
        funding_nights(
            utc("2026-07-01T12:00:00Z"),
            utc("2026-07-01T21:00:00Z"),
            cutoff(),
            tz
        ),
        1
    );
    assert_eq!(
        funding_nights(
            utc("2026-07-01T12:00:00Z"),
            utc("2026-07-01T20:59:00Z"),
            cutoff(),
            tz
        ),
        0
    );
    // A través del cambio de hora del 25-10-2026: sábado, domingo y lunes, sin contar de más.
    assert_eq!(
        funding_nights(
            utc("2026-10-24T12:00:00Z"),
            utc("2026-10-27T12:00:00Z"),
            cutoff(),
            tz
        ),
        3
    );
}

#[test]
fn a_reversed_interval_has_no_nights() {
    assert_eq!(
        funding_nights(
            utc("2026-11-20T10:00:00Z"),
            utc("2026-11-19T10:00:00Z"),
            cutoff(),
            chrono_tz::Europe::Madrid
        ),
        0
    );
}

#[test]
fn slippage_parameters_come_from_the_config() {
    // 0,25 × 1,6 + 2 × 0,1 = 0,6 de slippage; + 0,8 de medio spread.
    let c = CostConfig {
        slippage_spread_fraction: dec!(0.25),
        slippage_ticks: dec!(2),
    };
    assert_eq!(
        fill_price(Side::Long, Execution::Entry, dec!(100), &index(), &c),
        dec!(101.4)
    );
}

// ---------- Costos desde el catálogo (#75) ----------

mod from_catalog {
    use cfds_backtest::{Commission, InstrumentCosts};
    use cfds_market_data::Catalog;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/instruments.toml");

    fn costs(toml: &str, id: &str) -> Result<InstrumentCosts, Vec<(String, &'static str)>> {
        let c = Catalog::from_toml(toml).unwrap();
        InstrumentCosts::from_spec(&c.instruments[&id.parse().unwrap()])
            .map_err(|p| p.into_iter().map(|f| (f.item, f.field)).collect())
    }

    #[test]
    fn de40_costs_come_from_the_catalog() {
        assert_eq!(
            costs(REAL, "DE40").unwrap(),
            InstrumentCosts {
                spread: dec!(1.8),
                tick: dec!(0.1),
                commission: Commission::None,
                funding_admin_fee_pct: dec!(3),
                funding_day_count: dec!(360),
            }
        );
    }

    #[test]
    fn instruments_with_pending_terms_cannot_be_simulated() {
        // Los índices de EE. UU. esperan el tipo de referencia de la financiación (#67).
        for id in ["US100", "US500", "US30"] {
            assert_eq!(
                costs(REAL, id).unwrap_err(),
                [(id.to_string(), "funding_benchmark")]
            );
        }
    }

    #[test]
    fn any_missing_cost_blocks_and_is_reported() {
        let de40 = REAL.find("[instruments.DE40]").unwrap();
        for (field, from) in [
            (
                "tick",
                r#"tick = { value = "0.1", source = "ig-demo-api" }"#,
            ),
            (
                "spread_typical_points",
                r#"spread_typical_points = { value = "1.8", source = "ig-es-indices" }"#,
            ),
            (
                "funding_admin_fee_pct",
                r#"funding_admin_fee_pct = { value = "3", source = "ig-es-costes-indices" }"#,
            ),
        ] {
            let at = de40 + REAL[de40..].find(from).unwrap();
            let toml = format!(
                "{}{} = {{ pending = \"prueba\" }}{}",
                &REAL[..at],
                field,
                &REAL[at + from.len()..]
            );
            assert_eq!(
                costs(&toml, "DE40").unwrap_err(),
                [("DE40".to_string(), field)],
                "{field}"
            );
        }
    }

    #[test]
    fn the_epic_is_not_a_cost() {
        let de40 = REAL.find("[instruments.DE40]").unwrap();
        let from = r#"epic = { value = "IX.D.DAX.IFE.IP", source = "ig-demo-api" }"#;
        let at = de40 + REAL[de40..].find(from).unwrap();
        let toml = format!(
            "{}epic = {{ pending = \"x\" }}{}",
            &REAL[..at],
            &REAL[at + from.len()..]
        );
        assert!(costs(&toml, "DE40").is_ok());
    }

    #[test]
    fn shares_are_not_supported_yet() {
        let toml = REAL.replacen(r#"asset_class = "index""#, r#"asset_class = "share""#, 1);
        let errs = costs(&toml, "US100").unwrap_err();
        assert!(
            errs.contains(&("US100".to_string(), "commission")),
            "{errs:?}"
        );
    }
}
