//! Recorrido de cartera: varios instrumentos en una sola cuenta (spec §10).
//!
//! ```text
//! línea de tiempo = cierres de todas las velas de todos los instrumentos, en orden
//!   (misma hora: en el orden de `markets`)
//! en cada cierre τ:
//!   1. se realizan las operaciones cuya última salida terminó a τ o antes
//!   2. capital = realizado + valor de las abiertas a τ (salidas parciales ya hechas
//!      + el resto al último cierre de su instrumento)
//!   3. máximo del capital, y capital del día si cambió el día en Madrid
//!   4. si el instrumento de esta vela no tiene posición: señal → riesgo (margen libre =
//!      capital − margen de las abiertas; posiciones abiertas) → ejecución en t+1
//! ```
//! Con un solo instrumento es exactamente el recorrido de `runner::run` (hay un test).
//! El valor de las abiertas no descuenta todavía sus costos de salida ni la financiación:
//! esos se cobran al cerrar.
//!
//! Moneda (ADR 0013 §4): los montos de cada operación están en la moneda del instrumento y se
//! pasan a la de la cuenta con el tipo vigente en cada momento (`Setup::account_to_instrument`):
//! el valor de una abierta y su margen, al tipo de `τ`; el riesgo, al de la decisión; el P/L
//! realizado, al del cierre de su última salida. Sin tipo publicado, error: nunca se inventa.

use cfds_core::NewsInterpretation;
use cfds_core::{Candle, InstrumentId};
use cfds_indicators::wilder_atr;
use cfds_risk::{Account, OpenPosition, Rejection, Request, check, mark_peak};
use cfds_strategy::{Analysis, Side, news_adjustment};
use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use rust_decimal::Decimal;

use crate::{RejectedSignal, RunError, Setup, TradeRecord, choose, execute};

/// Un instrumento de la cartera. Todos comparten el riesgo, el capital inicial y la zona del día.
pub struct Market<'a> {
    pub setup: Setup,
    pub m15: &'a [Candle],
    pub h1: &'a [Candle],
    /// Si se puede abrir una operación al decidir en el cierre de cada vela (horario de
    /// entradas, spec §2 y §7.1). Vacío = sin restricción de horario: solo para series
    /// sintéticas; un recorrido con datos reales lo arma con `trading_window::flags`.
    pub entries: &'a [bool],
    /// Velas de cierre obligatorio (fin de la semana), del mismo lugar.
    pub force_close: &'a [bool],
    /// Momentos de error de datos (`cfds_market_data::check_series`, spec §7.8 y §12): después
    /// de cada uno no se abre hasta volver a tener el calentamiento completo. Vacío = datos sin
    /// problemas (series sintéticas).
    pub data_errors: &'a [DateTime<Utc>],
    /// Interpretaciones de noticias para el ajuste del puntaje (spec §9, #194). En el backtest,
    /// solo las de noticias publicadas después del corte del modelo intérprete (ADR 0008 §5).
    /// En cada vela cuenta solo lo interpretado hasta su cierre (`news_adjustment`). Vacío =
    /// sin ajuste de noticias.
    pub news: &'a [NewsInterpretation],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioTrade {
    pub instrument: InstrumentId,
    pub record: TradeRecord,
    /// Capital (realizado + no realizado) al decidir, en la moneda de la cuenta.
    pub equity_at_entry: Decimal,
    /// Margen libre al decidir (capital − margen de las otras posiciones abiertas).
    pub free_margin_at_entry: Decimal,
    /// `Sizing.risk` del módulo de riesgo: tamaño × (distancia al stop + colchón por gaps), en
    /// la moneda del instrumento (#689, para medir el efecto de los topes).
    pub sized_risk: Decimal,
    /// El tope por grupo correlacionado achicó el tamaño (`Sizing.group_capped`, #670).
    pub group_capped: bool,
    /// Factor de la caída gradual y la volatilidad objetivo (`Sizing.risk_scale`, #671).
    pub risk_scale: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioRejected {
    pub instrument: InstrumentId,
    pub time: DateTime<Utc>,
    pub rejected: RejectedSignal,
}

/// Un kill switch por drawdown, o su reactivación en el backtest (#277).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KillSwitchEvent {
    pub at: DateTime<Utc>,
    pub kind: KillSwitchEventKind,
    /// Capital (realizado + no realizado) en ese momento, en la moneda de la cuenta.
    pub equity: Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillSwitchEventKind {
    /// El drawdown llegó al límite: no se abre nada más.
    Activated,
    /// Solo en backtest: pasaron `kill_switch_reset_days` días hábiles y se reactiva, con el
    /// pico del drawdown desde el capital de ese momento.
    Reset,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortfolioReport {
    /// En el orden en que se abrieron.
    pub trades: Vec<PortfolioTrade>,
    pub rejected: Vec<PortfolioRejected>,
    /// La primera activación del kill switch, si hubo.
    pub kill_switch_at: Option<DateTime<Utc>>,
    /// Cada activación y cada reactivación, en orden (#277).
    pub kill_switch_events: Vec<KillSwitchEvent>,
    /// Capital final realizado, en la moneda de la cuenta.
    pub final_equity: Decimal,
    /// Capital (realizado + no realizado) al último cierre de cada día (zona del día). El
    /// último día termina con el capital final.
    pub daily_equity: Vec<(NaiveDate, Decimal)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortfolioError {
    Empty,
    /// Los instrumentos no comparten la misma cuenta (riesgo, capital inicial o zona del día).
    MixedSetup(&'static str),
    Run(RunError),
}

struct Open {
    market: usize,
    record: TradeRecord,
    /// Fin de la vela de la última salida.
    closes_at: DateTime<Utc>,
}

/// Tipo de cambio del instrumento de `setup` en `t`, o error si todavía no había ninguno.
fn fx(setup: &Setup, t: DateTime<Utc>) -> Result<Decimal, PortfolioError> {
    setup
        .account_to_instrument
        .at(t)
        .ok_or(PortfolioError::Run(RunError::MissingFxRate(t)))
}

fn sign(side: Side) -> Decimal {
    match side {
        Side::Long => Decimal::ONE,
        Side::Short => Decimal::NEGATIVE_ONE,
    }
}

/// Una operación está realizada cuando su última salida terminó a `now` o antes: al decidir en
/// ese cierre, ya no ocupa margen ni lugar.
fn is_closed(o: &Open, now: DateTime<Utc>) -> bool {
    o.closes_at <= now
}

/// Valor a `now` de una operación abierta, en la moneda del instrumento, sin costos de salida.
fn open_value(o: &Open, candles: &[Candle], now: DateTime<Utc>) -> Decimal {
    let r = &o.record;
    let s = sign(r.side);
    let mut value = Decimal::ZERO;
    let mut remaining = Decimal::ONE;
    for e in r.exits.iter().filter(|e| e.time <= now) {
        value += r.size * e.fraction * (e.price - r.entry_price) * s;
        remaining -= e.fraction;
    }
    // Último cierre conocido del instrumento a `now`.
    let closed = candles.partition_point(|c| c.end() <= now);
    if remaining > Decimal::ZERO && closed > r.entry_index {
        let mark = candles[closed - 1].close;
        value += r.size * remaining * (mark - r.entry_price) * s;
    }
    value
}

pub fn run_portfolio(markets: &[Market]) -> Result<PortfolioReport, PortfolioError> {
    let first = &markets.first().ok_or(PortfolioError::Empty)?.setup;
    for m in markets {
        if m.setup.risk != first.risk {
            return Err(PortfolioError::MixedSetup("riesgo"));
        }
        if m.setup.initial_equity != first.initial_equity {
            return Err(PortfolioError::MixedSetup("capital inicial"));
        }
        if m.setup.day_tz != first.day_tz {
            return Err(PortfolioError::MixedSetup("zona del día"));
        }
    }
    let analyses: Vec<Analysis> = markets
        .iter()
        .map(|m| {
            Analysis::new(m.m15, m.h1, m.setup.strategy).with_data_errors(m.data_errors.to_vec())
        })
        .collect();
    let atrs: Vec<Vec<Option<Decimal>>> = markets
        .iter()
        .map(|m| wilder_atr(m.m15, m.setup.strategy.patterns.candles.atr_period))
        .collect();

    // Línea de tiempo: (cierre, instrumento, vela) con vela t+1 disponible.
    let mut events: Vec<(DateTime<Utc>, usize, usize)> = markets
        .iter()
        .enumerate()
        .flat_map(|(mi, m)| {
            (0..m.m15.len().saturating_sub(1)).map(move |t| (m.m15[t].end(), mi, t))
        })
        .collect();
    events.sort();

    let mut report = PortfolioReport {
        trades: Vec::new(),
        rejected: Vec::new(),
        kill_switch_at: None,
        kill_switch_events: Vec::new(),
        final_equity: first.initial_equity,
        daily_equity: Vec::new(),
    };
    let mut cash = first.initial_equity;
    let mut peak = cash;
    let mut day: Option<NaiveDate> = None;
    let mut day_start = cash;
    let mut busy_until = vec![0usize; markets.len()];
    // Kill switch activo y días hábiles contados desde que se activó (#277).
    let mut killed = false;
    let mut business_days = 0u32;
    let mut open: Vec<Open> = Vec::new();

    for (now, mi, t) in events {
        // 1. Realizar lo que ya cerró, al tipo del cierre de su última salida.
        let mut still_open = Vec::with_capacity(open.len());
        for o in open {
            if is_closed(&o, now) {
                cash += o.record.net_pnl / fx(&markets[o.market].setup, o.closes_at)?;
            } else {
                still_open.push(o);
            }
        }
        open = still_open;
        // 2 y 3. Capital a `now`.
        let mut equity = cash;
        for o in &open {
            let setup = &markets[o.market].setup;
            equity += open_value(o, markets[o.market].m15, now) / fx(setup, now)?;
        }
        peak = mark_peak(peak, equity);
        let today = now.with_timezone(&first.day_tz).date_naive();
        if day != Some(today) {
            day = Some(today);
            day_start = equity;
            report.daily_equity.push((today, equity));
            // Solo en backtest (#277): después de N días hábiles se reactiva, y el drawdown se
            // mide de nuevo desde el capital de ese momento (si no, el pico viejo lo volvería a
            // disparar en la primera señal).
            if killed && !matches!(today.weekday(), Weekday::Sat | Weekday::Sun) {
                business_days += 1;
                if first
                    .backtest
                    .kill_switch_reset_days
                    .is_some_and(|n| business_days >= n)
                {
                    killed = false;
                    peak = equity;
                    report.kill_switch_events.push(KillSwitchEvent {
                        at: now,
                        kind: KillSwitchEventKind::Reset,
                        equity,
                    });
                }
            }
        } else if let Some(last) = report.daily_equity.last_mut() {
            last.1 = equity;
        }
        let in_window = markets[mi].entries.get(t).copied().unwrap_or(true);
        if killed || t < busy_until[mi] || !in_window {
            continue;
        }
        // 4. Decidir en este instrumento.
        let market = &markets[mi];
        let setup = &market.setup;
        // Ajuste de noticias con lo interpretado hasta el cierre de la vela (sin look-ahead).
        let at = market.m15[t].end();
        let news = |side| {
            news_adjustment(
                market.news,
                &setup.instrument,
                side,
                at,
                &setup.strategy.strategy.news,
                &setup.strategy.strategy.score,
            )
        };
        let Some(signal) =
            choose(&analyses[mi], t, news).map_err(|e| PortfolioError::Run(RunError::Score(e)))?
        else {
            continue;
        };
        let Some(atr_t) = atrs[mi][t] else {
            continue;
        };
        let mut used_margin = Decimal::ZERO;
        for o in &open {
            used_margin += o.record.margin / fx(&markets[o.market].setup, now)?;
        }
        let free_margin = equity - used_margin;
        let account = Account {
            equity,
            peak_equity: peak,
            day_start_equity: day_start,
            free_margin,
            kill_switch: false,
            open_positions: open
                .iter()
                .map(|o| {
                    let s = &markets[o.market].setup;
                    // Hasta el stop original con el tamaño completo: cota conservadora para el
                    // tope por grupo correlacionado (#670); sin grupos no se usa.
                    let adverse = match o.record.side {
                        Side::Long => o.record.entry_price - o.record.stop,
                        Side::Short => o.record.stop - o.record.entry_price,
                    }
                    .max(Decimal::ZERO);
                    Ok(OpenPosition {
                        instrument: s.instrument.clone(),
                        risk: o.record.size * adverse / fx(s, now)?,
                    })
                })
                .collect::<Result<Vec<_>, PortfolioError>>()?,
            // Volatilidad objetivo apagada en la config (#671): se conecta cuando se apruebe.
            realized_vol_pct: None,
        };
        let request = Request {
            instrument: setup.instrument.clone(),
            entry: signal.reference,
            stop: signal.stop,
            atr: atr_t,
            account_to_instrument: fx(setup, at)?,
        };
        let sizing = match check(&request, &account, &setup.limits, &setup.risk) {
            Ok(s) => s,
            Err(reason) => {
                if reason == Rejection::DrawdownLimit {
                    report.kill_switch_at.get_or_insert(now);
                    report.kill_switch_events.push(KillSwitchEvent {
                        at: now,
                        kind: KillSwitchEventKind::Activated,
                        equity,
                    });
                    killed = true;
                    business_days = 0;
                }
                report.rejected.push(PortfolioRejected {
                    instrument: setup.instrument.clone(),
                    time: now,
                    rejected: RejectedSignal {
                        index: t,
                        side: signal.side,
                        reason,
                    },
                });
                continue;
            }
        };
        let record = execute(
            &signal,
            sizing.size,
            sizing.leverage,
            sizing.margin,
            market.m15,
            &atrs[mi],
            market.force_close,
            setup,
        )
        .map_err(PortfolioError::Run)?;
        let last = record.exits.last().map_or(t + 1, |e| e.index);
        busy_until[mi] = last.max(t + 1);
        open.push(Open {
            market: mi,
            closes_at: market.m15[last].end(),
            record: record.clone(),
        });
        report.trades.push(PortfolioTrade {
            instrument: setup.instrument.clone(),
            record,
            equity_at_entry: equity,
            free_margin_at_entry: free_margin,
            sized_risk: sizing.risk,
            group_capped: sizing.group_capped,
            risk_scale: sizing.risk_scale,
        });
    }
    // Lo que quedó abierto ya tiene sus salidas (hasta el final de los datos): se realiza.
    for o in &open {
        cash += o.record.net_pnl / fx(&markets[o.market].setup, o.closes_at)?;
    }
    report.final_equity = cash;
    if let Some(last) = report.daily_equity.last_mut() {
        last.1 = cash;
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CloseReason, ExitReason, ExitRecord};
    use cfds_core::Timeframe;
    use rust_decimal::dec;

    fn at(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn candle(i: i64, close: Decimal) -> Candle {
        Candle {
            instrument_id: "US100".parse().unwrap(),
            timeframe: Timeframe::M15,
            start: at("2026-11-19T14:30:00Z") + chrono::TimeDelta::minutes(15 * i),
            open: close,
            high: close,
            low: close,
            close,
            volume: None,
        }
    }

    /// Largo de 2 por punto entrado a 100 en la vela 1; TP1 (la mitad) a 104 en la vela 2;
    /// el resto sale a 103 en la vela 4.
    fn open_trade() -> Open {
        let exit = |index: i64, price: Decimal, reason| ExitRecord {
            index: index as usize,
            time: candle(index, price).end(),
            mid: price,
            price,
            fraction: dec!(0.5),
            reason: CloseReason::Exit(reason),
        };
        Open {
            market: 0,
            closes_at: candle(4, dec!(0)).end(),
            record: TradeRecord {
                side: Side::Long,
                signal_index: 0,
                signal_at: candle(0, dec!(0)).end(),
                score: 5,
                breakdown: vec![],
                triggers: vec![],
                reference: dec!(100),
                tp1: dec!(104),
                entry_index: 1,
                entry_time: candle(1, dec!(0)).start,
                entry_mid: dec!(100),
                entry_price: dec!(100),
                stop: dec!(98),
                size: dec!(2),
                leverage: dec!(1),
                margin: dec!(10),
                exits: vec![
                    exit(2, dec!(104), ExitReason::Tp1),
                    exit(4, dec!(103), ExitReason::Trailing),
                ],
                commission: dec!(0),
                funding: dec!(0),
                mid_pnl: dec!(7),
                gross_pnl: dec!(7),
                net_pnl: dec!(7),
            },
        }
    }

    fn closes() -> Vec<Candle> {
        [100, 101, 104, 102, 103]
            .iter()
            .enumerate()
            .map(|(i, &c)| candle(i as i64, Decimal::from(c)))
            .collect()
    }

    #[test]
    fn open_value_marks_the_rest_and_keeps_partial_exits() {
        let (o, c) = (open_trade(), closes());
        // Al cierre de la vela 0 todavía no entró.
        assert_eq!(open_value(&o, &c, c[0].end()), dec!(0));
        // Vela 1: 2 × (101 − 100).
        assert_eq!(open_value(&o, &c, c[1].end()), dec!(2));
        // Vela 3: la mitad ya salió a 104 (+4) y el resto vale 1 × (102 − 100) = 2.
        assert_eq!(open_value(&o, &c, c[3].end()), dec!(6));
    }

    #[test]
    fn a_trade_is_realized_at_the_close_of_its_last_exit_candle() {
        let o = open_trade();
        let c = closes();
        assert!(!is_closed(&o, c[3].end()));
        assert!(is_closed(&o, c[4].end()));
    }
}
