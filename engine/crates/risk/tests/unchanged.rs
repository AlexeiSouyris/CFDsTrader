//! #670: con `config/risk.toml` tal como está en el repositorio (gaps en 0 y sin grupos), `check`
//! da exactamente lo mismo que antes, decimal por decimal, también con posiciones abiertas en los
//! índices de EE. UU. El runner de paper (#382) no cambia hasta que el dueño apruebe valores.
//! `legacy` es copia literal del cálculo anterior a #670.

use cfds_core::InstrumentId;
use cfds_risk::{Account, InstrumentLimits, OpenPosition, Rejection, Request, RiskConfig, check};
use rust_decimal::{Decimal, dec};

fn pct(x: Decimal) -> Decimal {
    x / Decimal::ONE_HUNDRED
}

/// (size, notional, leverage, margin, risk, leverage_capped) o el rechazo, como antes de #670.
type Legacy = Result<(Decimal, Decimal, Decimal, Decimal, Decimal, bool), Rejection>;

fn legacy(req: &Request, account: &Account, limits: &InstrumentLimits, cfg: &RiskConfig) -> Legacy {
    let positive = [
        (account.equity, "capital"),
        (account.peak_equity, "máximo del capital"),
        (account.day_start_equity, "capital al inicio del día"),
        (req.entry, "entrada"),
        (req.stop, "stop"),
        (req.atr, "ATR"),
        (req.account_to_instrument, "tipo de cambio"),
        (limits.max_leverage, "tope de apalancamiento"),
        (limits.margin_pct, "margen del instrumento"),
    ];
    if let Some((_, what)) = positive.iter().find(|(v, _)| *v <= Decimal::ZERO) {
        return Err(Rejection::InvalidInput(what));
    }
    if account.kill_switch {
        return Err(Rejection::KillSwitchActive);
    }
    let drawdown_floor = account.peak_equity * (Decimal::ONE - pct(cfg.max_drawdown_pct));
    if account.equity <= drawdown_floor {
        return Err(Rejection::DrawdownLimit);
    }
    let daily_floor = account.day_start_equity * (Decimal::ONE - pct(cfg.max_daily_loss_pct));
    if account.equity <= daily_floor {
        return Err(Rejection::DailyLossLimit);
    }
    if account.open_positions.len() >= cfg.max_open_positions {
        return Err(Rejection::MaxOpenPositions);
    }
    let same = account
        .open_positions
        .iter()
        .filter(|p| p.instrument == req.instrument)
        .count();
    if same >= cfg.max_positions_per_instrument {
        return Err(Rejection::InstrumentAlreadyOpen);
    }
    let r = (req.entry - req.stop).abs();
    if r.is_zero() {
        return Err(Rejection::InvalidInput("stop en la entrada"));
    }
    if r < cfg.min_stop_atr * req.atr {
        return Err(Rejection::StopTooClose);
    }
    if r > cfg.max_stop_atr * req.atr {
        return Err(Rejection::StopTooFar);
    }
    let fx = req.account_to_instrument;
    let equity = account.equity * fx;
    let mut size = pct(cfg.risk_per_trade_pct) * equity / r;
    let max_size = limits.max_leverage * equity / req.entry;
    let leverage_capped = size > max_size;
    if leverage_capped {
        size = max_size;
    }
    let notional = size * req.entry;
    let margin = notional * pct(limits.margin_pct);
    if margin > account.free_margin * fx {
        return Err(Rejection::InsufficientMargin);
    }
    match limits.min_deal_size {
        None => return Err(Rejection::MinDealSizeUnknown),
        Some(min) if size < min => return Err(Rejection::BelowMinDealSize),
        Some(_) => {}
    }
    Ok((
        size,
        notional,
        notional / equity,
        margin,
        size * r,
        leverage_capped,
    ))
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

#[test]
fn the_shipped_config_reproduces_the_previous_behavior_exactly() {
    let cfg = RiskConfig::from_toml(include_str!("../../../../config/risk.toml")).unwrap();
    let us = ["US100", "US500", "US30", "US2000"];
    let mut cases = 0;
    let mut accepted = 0;
    for (equity, peak, day_start) in [
        (dec!(10000), dec!(10000), dec!(10000)),
        (dec!(9000), dec!(10000), dec!(9100)),
        (dec!(8600), dec!(10000), dec!(8600)),
        (dec!(8400), dec!(10000), dec!(8400)),
        (dec!(9650), dec!(10000), dec!(10000)),
        (dec!(123456.78), dec!(130000), dec!(124000)),
    ] {
        for open in [
            vec![],
            vec![(us[1], dec!(100))],
            vec![(us[1], dec!(100)), (us[2], dec!(100)), (us[3], dec!(99.5))],
            vec![("DE40", dec!(50)), (us[0], dec!(100))],
            vec![
                (us[1], dec!(1)),
                (us[2], dec!(1)),
                (us[3], dec!(1)),
                ("DE40", dec!(1)),
            ],
            vec![
                (us[1], dec!(1)),
                (us[2], dec!(1)),
                (us[3], dec!(1)),
                ("DE40", dec!(1)),
                ("XAUUSD", dec!(1)),
            ],
        ] {
            for instrument in ["US100", "US500", "DE40", "XAUUSD"] {
                for (entry, stop, atr) in [
                    (dec!(20000), dec!(19950), dec!(30)),
                    (dec!(20000), dec!(20040), dec!(25)),
                    (dec!(5000.25), dec!(4990.5), dec!(7.3)),
                    (dec!(2650.1), dec!(2641.7), dec!(2.9)),
                    (dec!(100), dec!(99.9), dec!(2)),
                    (dec!(100), dec!(80), dec!(2)),
                ] {
                    for fx in [dec!(1), dec!(1.0834)] {
                        for (lev, margin, min) in [
                            (dec!(10), dec!(5), Some(dec!(0.5))),
                            (dec!(20), dec!(5), Some(dec!(0.01))),
                            (dec!(10), dec!(5), None),
                        ] {
                            let account = Account {
                                equity,
                                peak_equity: peak,
                                day_start_equity: day_start,
                                free_margin: equity / dec!(2),
                                kill_switch: false,
                                open_positions: open
                                    .iter()
                                    .map(|(i, risk)| OpenPosition {
                                        instrument: id(i),
                                        risk: *risk,
                                    })
                                    .collect(),
                            };
                            let req = Request {
                                instrument: id(instrument),
                                entry,
                                stop,
                                atr,
                                account_to_instrument: fx,
                            };
                            let limits = InstrumentLimits {
                                max_leverage: lev,
                                margin_pct: margin,
                                min_deal_size: min,
                            };
                            let now = check(&req, &account, &limits, &cfg);
                            let before = legacy(&req, &account, &limits, &cfg);
                            cases += 1;
                            match (&now, &before) {
                                (Ok(s), Ok(b)) => {
                                    accepted += 1;
                                    let got = (
                                        s.size,
                                        s.notional,
                                        s.leverage,
                                        s.margin,
                                        s.risk,
                                        s.leverage_capped,
                                    );
                                    assert_eq!(&got, b, "{req:?} {account:?}");
                                    // También la escala decimal (lo que se imprime y se guarda).
                                    assert_eq!(format!("{got:?}"), format!("{b:?}"));
                                    assert!(!s.group_capped);
                                }
                                (Err(e), Err(b)) => assert_eq!(e, b, "{req:?} {account:?}"),
                                _ => panic!("cambió: {now:?} vs {before:?} en {req:?} {account:?}"),
                            }
                        }
                    }
                }
            }
        }
    }
    // La grilla ejercita de verdad las dos ramas.
    assert!(
        cases > 2000 && accepted > 100,
        "{cases} casos, {accepted} aceptados"
    );
}
