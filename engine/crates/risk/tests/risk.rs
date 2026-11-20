//! Un test por límite que verifica que se dispara, con sus bordes (#49, Fase 6 del ROADMAP).

use cfds_core::InstrumentId;
use cfds_market_data::Catalog;
use cfds_risk::{Account, InstrumentLimits, Rejection, Request, RiskConfig, Sizing, check};
use rust_decimal::{Decimal, dec};

fn cfg() -> RiskConfig {
    RiskConfig::from_toml(include_str!("../../../../config/risk.toml")).unwrap()
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
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
    let others = |n: usize| -> Vec<InstrumentId> { (0..n).map(|i| id(&format!("X{i}"))).collect() };
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
        open_positions: vec![id("AAPL")],
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
    let catalog = Catalog::from_toml(include_str!("../../../../config/instruments.toml")).unwrap();
    let us100 = InstrumentLimits::from_spec(&catalog.instruments[&id("US100")]).unwrap();
    assert_eq!(
        us100,
        InstrumentLimits {
            max_leverage: dec!(10),
            margin_pct: dec!(5),
            min_deal_size: None,
        }
    );
}
