//! Tests de propiedades del módulo de riesgo (#663, #677; plan B1 de
//! `docs/analysis/2026-11-autoanalisis.md`).
//!
//! Para cualquier cuenta, instrumento, operación y configuración válidos (generados al azar,
//! también con colchón por gaps, grupos correlacionados, reducción por caída y volatilidad
//! objetivo), si `check` aprueba un tamaño:
//! - el riesgo (distancia al stop + colchón por gaps) nunca supera el presupuesto escalado
//!   (`risk_per_trade_pct × risk_scale` del capital);
//! - el riesgo nunca supera lo que queda libre en el grupo correlacionado más lleno;
//! - el apalancamiento nunca supera el tope del instrumento y el margen entra en el libre;
//! - el stop existe y está a una distancia válida (entre `min_atr` y `max_atr` ATR).
//!
//! Y nunca se aprueba nada con el kill switch activo ni con el stop en la entrada.

use cfds_core::InstrumentId;
use cfds_risk::config::{CorrelationGroup, DrawdownMode, DrawdownPoint};
use cfds_risk::{Account, InstrumentLimits, OpenPosition, Request, RiskConfig, check};
use proptest::prelude::*;
use rust_decimal::Decimal;

const CASES: u32 = 256;

/// La base: la config neutral (todo apagado) o la del repositorio (valores aprobados, #689).
fn base_cfg(repo: bool) -> RiskConfig {
    let text = if repo {
        include_str!("../../../../config/risk.toml")
    } else {
        include_str!("neutral_risk.toml")
    };
    RiskConfig::from_toml(text).unwrap()
}

fn id(s: &str) -> InstrumentId {
    s.parse().unwrap()
}

#[derive(Debug, Clone)]
struct Case {
    req: Request,
    account: Account,
    limits: InstrumentLimits,
    cfg: RiskConfig,
}

/// Configuración: la neutral o la del repositorio y, encima, al azar, con colchón por gaps, un grupo US500 + US100,
/// reducción por caída (lineal o en escalones) y volatilidad objetivo.
fn cfg() -> impl Strategy<Value = RiskConfig> {
    (
        any::<bool>(),                                           // base: repositorio o neutral
        prop_oneof![Just(0i64), 1i64..100],                      // gap_atr (centésimos)
        prop::option::of(50i64..400),                            // tope del grupo, % (centésimos)
        prop::option::of((any::<bool>(), 1i64..20, 10i64..100)), // caída: modo, desde %, factor (centésimos)
        prop::option::of((500i64..3_000, 20i64..100, 100i64..200)), // vol: objetivo, mín, máx (centésimos)
    )
        .prop_map(|(repo, gap, group, dd, vol)| {
            let mut c = base_cfg(repo);
            c.gap_atr = Decimal::new(gap, 2);
            if let Some(max) = group {
                c.groups = vec![CorrelationGroup {
                    name: "us".into(),
                    max_risk_pct: Decimal::new(max, 2),
                    instruments: vec![id("US500"), id("US100")],
                }];
            }
            if let Some((step, from, scale)) = dd {
                c.drawdown_scaling.mode = if step {
                    DrawdownMode::Step
                } else {
                    DrawdownMode::Linear
                };
                c.drawdown_scaling.points = vec![DrawdownPoint {
                    drawdown_pct: Decimal::from(from),
                    scale: Decimal::new(scale, 2),
                }];
            }
            if let Some((target, min, max)) = vol {
                c.vol_target.target_annual_pct = Decimal::new(target, 2);
                c.vol_target.min_scale = Decimal::new(min, 2);
                c.vol_target.max_scale = Decimal::new(max, 2);
            }
            c
        })
}

/// Una operación con el stop a `stop_atr` ATR (en centésimos), del lado largo o corto.
fn case() -> impl Strategy<Value = Case> {
    (
        (
            100_000i64..100_000_000,
            0i64..1_000,
            0i64..100_000,
            prop::option::of(100i64..5_000),
        ), // capital, caída ‰, + inicio del día, vol %
        (
            1_000i64..5_000_000,
            1i64..100_000,
            10i64..400,
            any::<bool>(),
        ), // entrada, ATR, stop en ATR, largo
        (
            5_000i64..20_000,
            prop::sample::select(vec![1i64, 2, 3, 5, 10, 20, 30]),
        ), // tipo de cambio, tope
        (50i64..5_000, 1i64..1_000), // margen %, mínimo
        prop::collection::vec(
            (
                prop::sample::select(vec!["US100", "DE40", "XAUUSD"]),
                0i64..100_000,
            ),
            0..4,
        ),
        cfg(),
    )
        .prop_map(
            |(
                (equity, dd, extra_day, vol),
                (entry, atr, stop_atr, long),
                (fx, lev),
                (margin, min),
                open,
                cfg,
            )| {
                let equity = Decimal::new(equity, 2);
                // Caída de hasta el 10 % (por debajo del límite del 15 %).
                let peak = equity / (Decimal::ONE - Decimal::new(dd / 10, 3));
                let entry = Decimal::new(entry, 2);
                let atr = Decimal::new(atr, 2);
                let distance = atr * Decimal::new(stop_atr, 2);
                Case {
                    req: Request {
                        instrument: id("US500"),
                        entry,
                        stop: if long {
                            entry - distance
                        } else {
                            entry + distance
                        },
                        atr,
                        account_to_instrument: Decimal::new(fx, 4),
                    },
                    account: Account {
                        equity,
                        peak_equity: peak.max(equity),
                        day_start_equity: equity + Decimal::new(extra_day, 2),
                        free_margin: equity,
                        kill_switch: false,
                        open_positions: open
                            .into_iter()
                            .map(|(i, risk)| OpenPosition {
                                instrument: id(i),
                                risk: Decimal::new(risk, 2),
                            })
                            .collect(),
                        realized_vol_pct: vol.map(|v| Decimal::new(v, 2)),
                    },
                    limits: InstrumentLimits {
                        max_leverage: Decimal::from(lev),
                        margin_pct: Decimal::new(margin, 2),
                        min_deal_size: Some(Decimal::new(min, 4)),
                    },
                    cfg,
                }
            },
        )
}

fn pct(x: Decimal) -> Decimal {
    x / Decimal::ONE_HUNDRED
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    #[test]
    fn approved_sizes_respect_every_limit(c in case()) {
        let Ok(s) = check(&c.req, &c.account, &c.limits, &c.cfg) else {
            return Ok(());
        };
        let fx = c.req.account_to_instrument;
        let equity = c.account.equity * fx;
        let r = (c.req.entry - c.req.stop).abs();
        let per_unit = r + c.cfg.gap_atr * c.req.atr;
        let budget = pct(c.cfg.risk_per_trade_pct * s.risk_scale) * equity;

        // Stop presente y a una distancia válida.
        prop_assert!(r > Decimal::ZERO);
        prop_assert!(r >= c.cfg.min_stop_atr * c.req.atr);
        prop_assert!(r <= c.cfg.max_stop_atr * c.req.atr);

        // Tamaño positivo y consistente; el riesgo incluye el colchón por gaps.
        prop_assert!(s.size > Decimal::ZERO);
        prop_assert_eq!(s.notional, s.size * c.req.entry);
        prop_assert_eq!(s.risk, s.size * per_unit);

        // Riesgo dentro del presupuesto escalado (la escala nunca agranda más allá de sus topes).
        prop_assert!(s.risk <= budget, "riesgo {} > presupuesto {}", s.risk, budget);
        // Riesgo dentro de lo libre del grupo (en la moneda del instrumento).
        for g in c.cfg.groups.iter().filter(|g| g.instruments.contains(&c.req.instrument)) {
            let used: Decimal = c.account.open_positions.iter()
                .filter(|p| g.instruments.contains(&p.instrument))
                .map(|p| p.risk)
                .sum();
            let room = (pct(g.max_risk_pct) * c.account.equity - used) * fx;
            prop_assert!(s.risk <= room, "riesgo {} > lugar del grupo {}", s.risk, room);
        }
        // Apalancamiento y margen.
        prop_assert!(s.leverage <= c.limits.max_leverage, "apalancamiento {}", s.leverage);
        prop_assert!(s.margin <= c.account.free_margin * fx);
        prop_assert!(s.size >= c.limits.min_deal_size.unwrap());

        // Si ningún tope recortó, se arriesga el presupuesto (como mucho unas cifras 28 menos).
        if !s.leverage_capped && !s.group_capped {
            prop_assert!(budget / per_unit - s.size <= Decimal::new(1, 20));
        }
    }

    #[test]
    fn nothing_is_approved_with_the_kill_switch_on(mut c in case()) {
        c.account.kill_switch = true;
        prop_assert!(check(&c.req, &c.account, &c.limits, &c.cfg).is_err());
    }

    #[test]
    fn nothing_is_approved_without_a_real_stop(mut c in case()) {
        c.req.stop = c.req.entry;
        prop_assert!(check(&c.req, &c.account, &c.limits, &c.cfg).is_err());
    }

    #[test]
    fn mirroring_the_stop_gives_the_same_size(c in case()) {
        // `check` mide |entrada − stop|: reflejar el stop al otro lado no cambia el tamaño.
        let mut mirrored = c.clone();
        mirrored.req.stop = c.req.entry + (c.req.entry - c.req.stop);
        prop_assume!(mirrored.req.stop > Decimal::ZERO && c.req.stop > Decimal::ZERO);
        prop_assert_eq!(
            check(&c.req, &c.account, &c.limits, &c.cfg),
            check(&mirrored.req, &mirrored.account, &mirrored.limits, &mirrored.cfg)
        );
    }

    #[test]
    fn a_tighter_leverage_cap_never_increases_the_size(c in case()) {
        let mut tighter = c.limits;
        tighter.max_leverage = c.limits.max_leverage / Decimal::TWO;
        if let (Ok(loose), Ok(tight)) = (
            check(&c.req, &c.account, &c.limits, &c.cfg),
            check(&c.req, &c.account, &tighter, &c.cfg),
        ) {
            prop_assert!(tight.size <= loose.size);
        }
    }
}

/// Caso mínimo que encontró `proptest` (#677): el riesgo quedaba en
/// 4824.1492462500000000000000001 con un presupuesto de 4824.14924625.
#[test]
fn rounding_never_pushes_the_risk_over_the_budget() {
    let cfg = base_cfg(false);
    let req = Request {
        instrument: id("US500"),
        entry: Decimal::new(1_000, 2),
        stop: Decimal::new(1_860_500, 4),
        atr: Decimal::new(35_210, 2),
        account_to_instrument: Decimal::new(8_475, 4),
    };
    let equity = Decimal::new(56_922_115, 2);
    let account = Account {
        equity,
        peak_equity: equity,
        day_start_equity: equity,
        free_margin: equity,
        kill_switch: false,
        open_positions: vec![],
        realized_vol_pct: None,
    };
    let limits = InstrumentLimits {
        max_leverage: Decimal::ONE,
        margin_pct: Decimal::new(50, 2),
        min_deal_size: Some(Decimal::new(1, 4)),
    };
    let s = check(&req, &account, &limits, &cfg).unwrap();
    let budget = pct(cfg.risk_per_trade_pct) * equity * req.account_to_instrument;
    assert!(s.risk <= budget, "{} > {}", s.risk, budget);
    assert!(s.leverage <= limits.max_leverage, "{}", s.leverage);
}
