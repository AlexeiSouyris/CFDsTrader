//! Un test por límite que verifica que se dispara, con sus bordes (#49, Fase 6 del ROADMAP).

use cfds_core::InstrumentId;
use cfds_market_data::Catalog;
use cfds_risk::{
    Account, CorrelationGroup, DrawdownMode, DrawdownPoint, DrawdownScaling, InstrumentLimits,
    OpenPosition, Rejection, Request, RiskConfig, Sizing, VolTarget, check,
};
use rust_decimal::{Decimal, dec};

/// Config neutral (todo lo de #670 y #671 apagado): cada test prende solo lo que mide.
fn cfg() -> RiskConfig {
    RiskConfig::from_toml(include_str!("neutral_risk.toml")).unwrap()
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

/// Posición abierta en `s` que arriesga `risk` (moneda de la cuenta).
fn pos(s: &str, risk: Decimal) -> OpenPosition {
    OpenPosition {
        instrument: id(s),
        risk,
    }
}

/// Cuenta de 1.000 sin posiciones ni pérdidas.
fn account() -> Account {
    Account {
        equity: dec!(1000),
        peak_equity: dec!(1000),
        day_start_equity: dec!(1000),
        free_margin: dec!(1000),
        kill_switch: false,
        open_positions: vec![],
        realized_vol_pct: None,
    }
}

/// Entrada 100, stop 98 (R = 2 = 1 × ATR), misma moneda.
fn request() -> Request {
    Request {
        instrument: id("AAPL"),
        entry: dec!(100),
        stop: dec!(98),
        atr: dec!(2),
        account_to_instrument: dec!(1),
    }
}

fn limits() -> InstrumentLimits {
    InstrumentLimits {
        max_leverage: dec!(3),
        margin_pct: dec!(20),
        min_deal_size: Some(dec!(0.1)),
    }
}

fn run(req: &Request, acc: &Account, lim: &InstrumentLimits) -> Result<Sizing, Rejection> {
    check(req, acc, lim, &cfg())
}

// ---------- Tamaño ----------

#[test]
fn risks_one_percent_of_equity() {
    // 1 % de 1.000 = 10; R = 2 → 5 por punto; nocional 500; apalancamiento 0,5; margen 20 % = 100.
    let s = run(&request(), &account(), &limits()).unwrap();
    assert_eq!(
        s,
        Sizing {
            size: dec!(5),
            notional: dec!(500),
            leverage: dec!(0.5),
            margin: dec!(100),
            risk: dec!(10),
            leverage_capped: false,
            group_capped: false,
            risk_scale: dec!(1),
        }
    );
}

#[test]
fn short_is_the_mirror() {
    let req = Request {
        stop: dec!(102),
        ..request()
    };
    assert_eq!(
        run(&req, &account(), &limits()),
        run(&request(), &account(), &limits())
    );
}

#[test]
fn the_leverage_cap_reduces_the_size() {
    // Índice a 20.000 con R = 10: sin tope serían 1 por punto (nocional 20.000, 20:1).
    let req = Request {
        instrument: id("US100"),
        entry: dec!(20000),
        stop: dec!(19990),
        atr: dec!(10),
        ..request()
    };
    let lim = InstrumentLimits {
        max_leverage: dec!(10),
        margin_pct: dec!(5),
        ..limits()
    };
    let s = run(&req, &account(), &lim).unwrap();
    assert!(s.leverage_capped);
    assert_eq!(
        (s.size, s.notional, s.leverage, s.risk),
        (dec!(0.5), dec!(10000), dec!(10), dec!(5))
    );
    // El riesgo queda por debajo del 1 %.
    assert!(s.risk < dec!(10));
    // Exactamente en el tope no se recorta.
    let at_cap = InstrumentLimits {
        max_leverage: dec!(20),
        ..lim
    };
    let s = run(&req, &account(), &at_cap).unwrap();
    assert_eq!((s.leverage_capped, s.leverage), (false, dec!(20)));
}

#[test]
fn sizes_in_the_instrument_currency() {
    // 1.000 EUR a 1,10 USD/EUR = 1.100 USD; 1 % = 11 USD; R = 2 → 5,5 USD por punto.
    let req = Request {
        account_to_instrument: dec!(1.1),
        ..request()
    };
    let s = run(&req, &account(), &limits()).unwrap();
    assert_eq!(
        (s.size, s.risk, s.leverage),
        (dec!(5.5), dec!(11), dec!(0.5))
    );
}

// ---------- Límites de la cuenta ----------

#[test]
fn kill_switch_blocks_everything() {
    let acc = Account {
        kill_switch: true,
        ..account()
    };
    assert_eq!(
        run(&request(), &acc, &limits()),
        Err(Rejection::KillSwitchActive)
    );
}

#[test]
fn drawdown_of_15_percent_triggers_the_kill_switch() {
    let at = Account {
        equity: dec!(850),
        day_start_equity: dec!(850),
        ..account()
    };
    assert_eq!(
        run(&request(), &at, &limits()),
        Err(Rejection::DrawdownLimit)
    );
    let above = Account {
        equity: dec!(850.01),
        day_start_equity: dec!(850.01),
        ..account()
    };
    assert!(run(&request(), &above, &limits()).is_ok());
}

#[test]
fn daily_loss_of_3_percent_stops_new_entries() {
    let at = Account {
        equity: dec!(970),
        ..account()
    };
    assert_eq!(
        run(&request(), &at, &limits()),
        Err(Rejection::DailyLossLimit)
    );
    let above = Account {
        equity: dec!(970.01),
        ..account()
    };
    assert!(run(&request(), &above, &limits()).is_ok());
    // Se mide desde el capital de las 00:00, no desde el máximo histórico.
    let new_day = Account {
        equity: dec!(970),
        day_start_equity: dec!(975),
        ..account()
    };
    assert!(run(&request(), &new_day, &limits()).is_ok());
}

#[test]
fn at_most_5_open_positions() {
    let others = |n: usize| -> Vec<OpenPosition> {
        (0..n).map(|i| pos(&format!("X{i}"), dec!(10))).collect()
    };
    let five = Account {
        open_positions: others(5),
        ..account()
    };
    assert_eq!(
        run(&request(), &five, &limits()),
        Err(Rejection::MaxOpenPositions)
    );
    let four = Account {
        open_positions: others(4),
        ..account()
    };
    assert!(run(&request(), &four, &limits()).is_ok());
}

#[test]
fn one_position_per_instrument() {
    let acc = Account {
        open_positions: vec![pos("AAPL", dec!(10))],
        ..account()
    };
    assert_eq!(
        run(&request(), &acc, &limits()),
        Err(Rejection::InstrumentAlreadyOpen)
    );
}

// ---------- Distancia al stop, margen y mínimo ----------

#[test]
fn stop_distance_between_half_and_three_atr() {
    let with_atr = |atr: Decimal| Request { atr, ..request() };
    // R = 2: con ATR 4 es 0,5 × ATR (vale); con ATR 4,01 queda por debajo.
    assert!(run(&with_atr(dec!(4)), &account(), &limits()).is_ok());
    assert_eq!(
        run(&with_atr(dec!(4.01)), &account(), &limits()),
        Err(Rejection::StopTooClose)
    );
    // Con ATR 1, R = 3 es exactamente 3 × ATR (vale); R = 3,01 se pasa.
    let far = |stop: Decimal| Request {
        stop,
        atr: dec!(1),
        ..request()
    };
    assert!(run(&far(dec!(97)), &account(), &limits()).is_ok());
    assert_eq!(
        run(&far(dec!(96.99)), &account(), &limits()),
        Err(Rejection::StopTooFar)
    );
}

#[test]
fn margin_must_fit_in_free_margin() {
    // El margen requerido es 100.
    let exact = Account {
        free_margin: dec!(100),
        ..account()
    };
    assert!(run(&request(), &exact, &limits()).is_ok());
    let short = Account {
        free_margin: dec!(99.99),
        ..account()
    };
    assert_eq!(
        run(&request(), &short, &limits()),
        Err(Rejection::InsufficientMargin)
    );
}

#[test]
fn below_the_broker_minimum_does_not_open() {
    // El tamaño es 5.
    let exact = InstrumentLimits {
        min_deal_size: Some(dec!(5)),
        ..limits()
    };
    assert!(run(&request(), &account(), &exact).is_ok());
    let above = InstrumentLimits {
        min_deal_size: Some(dec!(5.01)),
        ..limits()
    };
    assert_eq!(
        run(&request(), &account(), &above),
        Err(Rejection::BelowMinDealSize)
    );
}

#[test]
fn an_unconfirmed_broker_minimum_does_not_open() {
    let unknown = InstrumentLimits {
        min_deal_size: None,
        ..limits()
    };
    assert_eq!(
        run(&request(), &account(), &unknown),
        Err(Rejection::MinDealSizeUnknown)
    );
}

#[test]
fn impossible_inputs_are_rejected() {
    let bad = [
        Request {
            stop: dec!(100),
            ..request()
        },
        Request {
            entry: dec!(0),
            ..request()
        },
        Request {
            atr: dec!(0),
            ..request()
        },
        Request {
            account_to_instrument: dec!(-1),
            ..request()
        },
    ];
    for req in bad {
        assert!(
            matches!(
                run(&req, &account(), &limits()),
                Err(Rejection::InvalidInput(_))
            ),
            "{req:?}"
        );
    }
    let broke = Account {
        equity: dec!(0),
        ..account()
    };
    assert!(matches!(
        run(&request(), &broke, &limits()),
        Err(Rejection::InvalidInput(_))
    ));
}

// ---------- Catálogo ----------

#[test]
fn limits_come_from_the_catalog_and_pending_minimums_stay_unknown() {
    let real = include_str!("../../../../config/instruments.toml");
    let catalog = Catalog::from_toml(real).unwrap();
    let us100 = InstrumentLimits::from_spec(&catalog.instruments[&id("US100")]).unwrap();
    assert_eq!(
        us100,
        InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            // Contrato de 1 € por punto, leído de la API de IG (#67).
            min_deal_size: Some(dec!(0.5)),
        }
    );

    // Un mínimo pendiente queda desconocido: no se inventa.
    let pending = real.replacen(
        r#"min_deal_size = { value = "0.5", source = "ig-demo-api" }"#,
        r#"min_deal_size = { pending = "sin confirmar" }"#,
        1,
    );
    assert_ne!(pending, real);
    let catalog = Catalog::from_toml(&pending).unwrap();
    let us100 = InstrumentLimits::from_spec(&catalog.instruments[&id("US100")]).unwrap();
    assert_eq!(us100.min_deal_size, None);
}

#[test]
fn outside_the_backtest_the_kill_switch_never_resets_by_itself() {
    // #277: la reactivación después de N días hábiles existe solo en el backtest
    // (`config/backtest.toml`, `cfds_backtest`). El módulo de riesgo, que usa la operación en vivo,
    // no tiene ese parámetro: la configuración lo rechaza en cualquier sección.
    let real = include_str!("../../../../config/risk.toml");
    for bad in [
        format!("{real}\n[kill_switch]\nreset_after_business_days = 20\n"),
        real.replace(
            "max_drawdown_pct = \"15\"",
            "max_drawdown_pct = \"15\"\nreset_after_business_days = 20",
        ),
    ] {
        assert_ne!(bad, real);
        assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
    }
    // Y con el kill switch activo, nada lo saltea: ni un capital nuevo ni un pico recuperado.
    for (equity, peak) in [(dec!(1000), dec!(1000)), (dec!(2000), dec!(1000))] {
        let acc = Account {
            kill_switch: true,
            equity,
            peak_equity: peak,
            day_start_equity: equity,
            ..account()
        };
        assert_eq!(
            run(&request(), &acc, &limits()),
            Err(Rejection::KillSwitchActive)
        );
    }
}

// ---------- Grupos correlacionados y gaps (#670) ----------

/// Config del repositorio con un grupo AAPL/MSFT/NVDA con tope del 1,5 % (15 sobre 1.000).
fn with_group() -> RiskConfig {
    RiskConfig {
        groups: vec![CorrelationGroup {
            name: "tech".into(),
            max_risk_pct: dec!(1.5),
            instruments: vec![id("AAPL"), id("MSFT"), id("NVDA")],
        }],
        ..cfg()
    }
}

fn with_open(open: Vec<OpenPosition>) -> Account {
    Account {
        open_positions: open,
        ..account()
    }
}

#[test]
fn the_group_cap_has_room_for_a_full_trade() {
    // 4 abiertos en el grupo + 10 nuevos = 14 ≤ 15: tamaño completo.
    let s = check(
        &request(),
        &with_open(vec![pos("MSFT", dec!(4))]),
        &limits(),
        &with_group(),
    )
    .unwrap();
    assert_eq!((s.size, s.risk, s.group_capped), (dec!(5), dec!(10), false));
}

#[test]
fn the_group_cap_reduces_the_size_to_the_room_left() {
    // 9 abiertos: quedan 6 de 15 → tamaño 3 (R = 2).
    let s = check(
        &request(),
        &with_open(vec![pos("MSFT", dec!(5)), pos("NVDA", dec!(4))]),
        &limits(),
        &with_group(),
    )
    .unwrap();
    assert_eq!((s.size, s.risk, s.group_capped), (dec!(3), dec!(6), true));
    assert!(!s.leverage_capped);
}

#[test]
fn a_full_group_rejects_with_its_own_reason() {
    for used in [dec!(15), dec!(20)] {
        assert_eq!(
            check(
                &request(),
                &with_open(vec![pos("MSFT", used)]),
                &limits(),
                &with_group()
            ),
            Err(Rejection::GroupRiskLimit),
            "{used}"
        );
    }
    // Justo por debajo del tope todavía entra, achicada.
    let s = check(
        &request(),
        &with_open(vec![pos("MSFT", dec!(14))]),
        &limits(),
        &with_group(),
    )
    .unwrap();
    assert_eq!((s.size, s.risk, s.group_capped), (dec!(0.5), dec!(1), true));
}

#[test]
fn positions_outside_the_group_do_not_count() {
    let acc = with_open(vec![pos("DE40", dec!(50)), pos("XAUUSD", dec!(50))]);
    let s = check(&request(), &acc, &limits(), &with_group()).unwrap();
    assert_eq!((s.size, s.group_capped), (dec!(5), false));
    // Un instrumento sin grupo no mira los grupos aunque estén llenos.
    let req = Request {
        instrument: id("DE40"),
        ..request()
    };
    let full = with_open(vec![pos("MSFT", dec!(100))]);
    assert_eq!(
        check(&req, &full, &limits(), &with_group()).map(|s| s.size),
        Ok(dec!(5))
    );
}

#[test]
fn the_fullest_group_wins() {
    // AAPL está en dos grupos: el de tech tiene 6 libres, el de "mega" 2 → manda el de 2.
    let mut cfg = with_group();
    cfg.groups.push(CorrelationGroup {
        name: "mega".into(),
        max_risk_pct: dec!(1),
        instruments: vec![id("AAPL"), id("GOOG")],
    });
    let acc = with_open(vec![pos("MSFT", dec!(9)), pos("GOOG", dec!(8))]);
    let s = check(&request(), &acc, &limits(), &cfg).unwrap();
    assert_eq!((s.risk, s.group_capped), (dec!(2), true));
}

#[test]
fn the_group_room_is_in_the_account_currency() {
    // 1.000 EUR a 1,10: quedan 6 EUR de lugar = 6,6 USD → tamaño 3,3 USD por punto.
    let req = Request {
        account_to_instrument: dec!(1.1),
        ..request()
    };
    let acc = with_open(vec![pos("MSFT", dec!(9))]);
    let s = check(&req, &acc, &limits(), &with_group()).unwrap();
    assert_eq!((s.size, s.risk), (dec!(3.3), dec!(6.6)));
}

#[test]
fn the_group_cap_still_respects_the_other_limits() {
    // Con lugar parcial, el mínimo del broker sigue mandando.
    let lim = InstrumentLimits {
        min_deal_size: Some(dec!(4)),
        ..limits()
    };
    let acc = with_open(vec![pos("MSFT", dec!(9))]);
    assert_eq!(
        check(&request(), &acc, &lim, &with_group()),
        Err(Rejection::BelowMinDealSize)
    );
    // Y los límites de la cuenta se miran antes que el grupo.
    let killed = Account {
        kill_switch: true,
        ..with_open(vec![pos("MSFT", dec!(100))])
    };
    assert_eq!(
        check(&request(), &killed, &limits(), &with_group()),
        Err(Rejection::KillSwitchActive)
    );
}

#[test]
fn a_negative_open_risk_is_invalid() {
    let acc = with_open(vec![pos("MSFT", dec!(-1))]);
    assert!(matches!(
        check(&request(), &acc, &limits(), &with_group()),
        Err(Rejection::InvalidInput(_))
    ));
}

#[test]
fn the_gap_allowance_shrinks_the_size_and_counts_in_the_risk() {
    // R = 2 y ATR = 2; colchón de 0,5 ATR = 1 → 10 / 3 por punto; el riesgo total sigue en 10.
    let cfg = RiskConfig {
        gap_atr: dec!(0.5),
        ..cfg()
    };
    let s = check(&request(), &account(), &limits(), &cfg).unwrap();
    assert_eq!(s.size, dec!(10) / dec!(3));
    assert_eq!(s.risk.round_dp(10), dec!(10));
    // La distancia al stop sigue siendo la misma para los límites en ATR.
    let far = Request {
        stop: dec!(97),
        atr: dec!(1),
        ..request()
    };
    assert!(check(&far, &account(), &limits(), &cfg).is_ok());
}

#[test]
fn gap_and_group_combine() {
    // Colchón 0,5 ATR (por unidad 3) y 9 abiertos: lugar 6 → tamaño 2, riesgo 6.
    let cfg = RiskConfig {
        gap_atr: dec!(0.5),
        ..with_group()
    };
    let acc = with_open(vec![pos("MSFT", dec!(9))]);
    let s = check(&request(), &acc, &limits(), &cfg).unwrap();
    assert_eq!((s.size, s.risk, s.group_capped), (dec!(2), dec!(6), true));
}

// ---------- Caída gradual y volatilidad objetivo (#671) ----------

/// 5 % → factor 1, 10 % → 0,5, lineal entre medio.
fn with_drawdown(mode: DrawdownMode) -> RiskConfig {
    RiskConfig {
        drawdown_scaling: DrawdownScaling {
            mode,
            points: vec![
                DrawdownPoint {
                    drawdown_pct: dec!(5),
                    scale: dec!(1),
                },
                DrawdownPoint {
                    drawdown_pct: dec!(10),
                    scale: dec!(0.5),
                },
            ],
        },
        ..cfg()
    }
}

fn in_drawdown(equity: Decimal) -> Account {
    Account {
        equity,
        day_start_equity: equity,
        free_margin: equity,
        ..account()
    }
}

#[test]
fn drawdown_scaling_lowers_the_risk_gradually() {
    let cfg = with_drawdown(DrawdownMode::Linear);
    // 7,5 % de caída: factor 0,75 → 0,75 % de 925 = 6,9375; R = 2.
    let s = check(&request(), &in_drawdown(dec!(925)), &limits(), &cfg).unwrap();
    assert_eq!((s.risk_scale, s.risk), (dec!(0.75), dec!(6.9375)));
    // Sin caída, todo igual.
    let s = check(&request(), &account(), &limits(), &cfg).unwrap();
    assert_eq!((s.risk_scale, s.risk), (dec!(1), dec!(10)));
    // Más allá del último punto, el último factor; al 15 % sigue el kill switch.
    let s = check(&request(), &in_drawdown(dec!(870)), &limits(), &cfg).unwrap();
    assert_eq!(s.risk_scale, dec!(0.5));
    assert_eq!(
        check(&request(), &in_drawdown(dec!(850)), &limits(), &cfg),
        Err(Rejection::DrawdownLimit)
    );
    // En escalones, 7,5 % todavía es factor 1.
    let step = with_drawdown(DrawdownMode::Step);
    let s = check(&request(), &in_drawdown(dec!(925)), &limits(), &step).unwrap();
    assert_eq!(s.risk_scale, dec!(1));
}

#[test]
fn vol_target_scales_the_risk_with_caps() {
    let cfg = RiskConfig {
        vol_target: VolTarget {
            target_annual_pct: dec!(10),
            min_scale: dec!(0.5),
            max_scale: dec!(1),
        },
        ..cfg()
    };
    let with_vol = |v: Option<Decimal>| Account {
        realized_vol_pct: v,
        ..account()
    };
    for (vol, scale) in [
        (Some(dec!(20)), dec!(0.5)),
        (Some(dec!(12.5)), dec!(0.8)),
        (Some(dec!(40)), dec!(0.5)),
        (Some(dec!(5)), dec!(1)),
        (None, dec!(1)),
    ] {
        let s = check(&request(), &with_vol(vol), &limits(), &cfg).unwrap();
        assert_eq!((s.risk_scale, s.risk), (scale, dec!(10) * scale), "{vol:?}");
    }
}

#[test]
fn drawdown_and_vol_multiply_and_the_group_cap_still_applies() {
    let mut cfg = with_drawdown(DrawdownMode::Linear);
    cfg.vol_target = VolTarget {
        target_annual_pct: dec!(10),
        min_scale: dec!(0.5),
        max_scale: dec!(1),
    };
    cfg.groups = with_group().groups;
    // Caída 7,5 % (0,75) × vol 12,5 % (0,8) = 0,6 → 0,6 % de 925 = 5,55; el grupo deja 15 × 0,925
    // − 9 = 4,875 → manda el grupo.
    let acc = Account {
        realized_vol_pct: Some(dec!(12.5)),
        open_positions: vec![pos("MSFT", dec!(9))],
        ..in_drawdown(dec!(925))
    };
    let s = check(&request(), &acc, &limits(), &cfg).unwrap();
    assert_eq!(s.risk_scale, dec!(0.6));
    assert!(s.group_capped);
    assert_eq!(s.risk, dec!(4.875));
}
