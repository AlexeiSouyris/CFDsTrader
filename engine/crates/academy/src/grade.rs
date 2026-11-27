//! Calificación de una sesión de replay terminada (`exam_result.v1`, §5.4 del diseño, #722).
//! Función pura: la misma sesión con la misma config da siempre lo mismo.
//!
//! ```text
//! proceso (aprueba o reprueba), por entrada decidida al cierre de t con lo que se sabía en t:
//!   stop_present, stop_side (contra el cierre de t), stop_in_range (min..max × ATR(t), risk.toml),
//!   risk_within_limit (risk_pct <= el del ticket), plan_written, one_position (ninguna anterior
//!   abierta en t)
//! sesión: enough_trades (examen de operaciones), level_precision (examen de niveles),
//!   enough_pivots y pivot_precision (examen de pivotes, #800)
//! niveles: los soportes y resistencias del motor (patterns) con las velas hasta el cursor final;
//!   una marca acierta si queda a level_tolerance_atr × ATR(cursor) de un nivel del motor
//! pivotes: los pivotes confirmados del motor (fuerza N de patterns) con las velas hasta el cursor
//!   final; una marca acierta en la misma vela y del mismo tipo, marcada con el cursor en la vela
//!   de confirmación (índice + N) o después; antes es «adelantada» (era una suposición)
//! resultado (se muestra, NO aprueba): entrada en la apertura de t+1, stop / gap / objetivo / fin
//!   con `simulate_leg` del backtest; medio spread y slippage con `fill_price`; financiación por
//!   noche con las velas del futuro = solo la tasa administrativa (la referencia ya la descuenta
//!   la base, ADR 0015, #467). R = distancia del cierre de t al stop.
//! ```

use cfds_backtest::research::StudyExit;
use cfds_backtest::research::run::simulate_leg;
use cfds_backtest::{
    Commission, CostConfig, Execution, InstrumentCosts, fill_price, funding, funding_nights,
};
use cfds_core::Timeframe;
use cfds_indicators::wilder_atr;
use cfds_patterns::PatternsConfig;
use cfds_patterns::levels::levels_at;
use cfds_patterns::pivots::{PivotKind, confirmed_pivots};
use cfds_strategy::Side;
use chrono::{DateTime, NaiveTime, Utc};
use chrono_tz::Tz;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::{Deserialize, Serialize};

use crate::replay::{
    MarkKind, OrderSide, ProcessRules, ReplayConfig, ReplayError, ReplayKind, ReplaySession,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rule {
    StopPresent,
    StopSide,
    StopInRange,
    RiskWithinLimit,
    PlanWritten,
    OnePosition,
    EnoughTrades,
    LevelPrecision,
    /// Al menos `pivots_per_exam` pivotes marcados (#800).
    EnoughPivots,
    /// Precisión de pivotes >= el umbral de preguntas (#800).
    PivotPrecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleCheck {
    pub rule: Rule,
    pub passed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExitKind {
    Stop,
    StopGap,
    Target,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub entry_index: usize,
    pub entry_price: Decimal,
    pub exit_index: usize,
    pub exit_price: Decimal,
    pub exit: ExitKind,
    #[serde(deserialize_with = "Option::deserialize")]
    pub r_gross: Option<Decimal>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub costs_r: Option<Decimal>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub r_net: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TradeGrade {
    pub order_id: u32,
    pub rules: Vec<RuleCheck>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub outcome: Option<Outcome>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelGrade {
    pub marked: usize,
    pub engine: usize,
    pub matched_marks: usize,
    pub covered_levels: usize,
    #[serde(deserialize_with = "Option::deserialize")]
    pub precision_pct: Option<Decimal>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub coverage_pct: Option<Decimal>,
    pub tolerance: Decimal,
    pub engine_levels: Vec<Decimal>,
}

/// Cómo quedó una marca de pivote (#800).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PivotStatus {
    /// En la vela y del tipo de un pivote del motor, marcada cuando ya estaba confirmado.
    Hit,
    /// En la vela de un pivote del motor, pero marcada antes de su confirmación.
    Early,
    /// Ningún pivote del motor confirmado hasta el cursor final en esa vela y de ese tipo.
    Miss,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PivotMarkGrade {
    pub mark_id: u32,
    pub status: PivotStatus,
    /// La vela de confirmación del pivote del motor (para explicar una adelantada); null en `miss`.
    #[serde(deserialize_with = "Option::deserialize")]
    pub confirmed_at: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnginePivot {
    pub bar_index: usize,
    pub kind: MarkKind,
    pub confirmed_at: usize,
}

/// `exam_result.v1#/properties/pivots`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PivotGrade {
    pub strength: usize,
    pub marked: usize,
    pub hits: usize,
    pub early: usize,
    pub engine: usize,
    pub covered: usize,
    #[serde(deserialize_with = "Option::deserialize")]
    pub precision_pct: Option<Decimal>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub coverage_pct: Option<Decimal>,
    pub marks: Vec<PivotMarkGrade>,
    pub engine_pivots: Vec<EnginePivot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revealed {
    pub instrument_id: cfds_core::InstrumentId,
    pub timeframe: Timeframe,
    pub source: String,
    pub first_bar_at: DateTime<Utc>,
    pub last_bar_at: DateTime<Utc>,
}

/// `contracts/exam_result.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExamResult {
    pub session_id: String,
    pub kind: ReplayKind,
    #[serde(deserialize_with = "Option::deserialize")]
    pub lesson_id: Option<String>,
    pub catalog_version: String,
    #[serde(deserialize_with = "Option::deserialize")]
    pub passed: Option<bool>,
    pub rules: Vec<RuleCheck>,
    pub trades: Vec<TradeGrade>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub levels: Option<LevelGrade>,
    /// Opcional en el contrato (llegó con #800): los resultados guardados antes no lo traen.
    #[serde(default)]
    pub pivots: Option<PivotGrade>,
    pub revealed: Revealed,
    pub graded_at: DateTime<Utc>,
}

/// Costos del CFD del catálogo para el resultado en R.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplayCosts {
    pub costs: InstrumentCosts,
    pub execution: CostConfig,
    pub funding_cutoff: NaiveTime,
    pub funding_tz: Tz,
}

/// Todo lo que la calificación lee de config.
#[derive(Debug, Clone, Copy)]
pub struct GradeConfig<'a> {
    pub replay: &'a ReplayConfig,
    pub rules: ProcessRules,
    pub patterns: &'a PatternsConfig,
    /// El mismo umbral que los exámenes de preguntas, para la precisión de niveles.
    pub pass_pct: Decimal,
    pub catalog_version: &'a str,
}

fn pct(n: usize, d: usize) -> Option<Decimal> {
    (d > 0).then(|| {
        (Decimal::from(n) * Decimal::ONE_HUNDRED / Decimal::from(d))
            .round_dp_with_strategy(2, RoundingStrategy::ToZero)
            .normalize()
    })
}

fn check(rule: Rule, passed: bool) -> RuleCheck {
    RuleCheck { rule, passed }
}

/// Califica una sesión **terminada**. El resultado en R necesita un instrumento sin comisión
/// por ejecución (índices): con comisión, el R dependería del tamaño, que el replay no tiene.
pub fn grade(
    s: &ReplaySession,
    cfg: GradeConfig,
    costs: &ReplayCosts,
    graded_at: DateTime<Utc>,
) -> Result<ExamResult, ReplayError> {
    let bars = s.all_bars()?;
    if costs.costs.commission != Commission::None {
        return Err(ReplayError::Bad(
            "el replay solo califica instrumentos sin comisión por ejecución".into(),
        ));
    }
    let atr = wilder_atr(bars, cfg.patterns.candles.atr_period);
    let last = bars.len() - 1;
    let mut trades = Vec::new();
    let mut exits: Vec<(usize, usize)> = Vec::new(); // (decisión, salida) de las anteriores
    for o in &s.orders {
        let t = o.cursor;
        let reference = bars[t].close;
        let side = match o.side {
            OrderSide::Long => Side::Long,
            OrderSide::Short => Side::Short,
        };
        let sign = if side == Side::Long {
            Decimal::ONE
        } else {
            Decimal::NEGATIVE_ONE
        };
        let distance = o.stop.map(|stop| (reference - stop).abs());
        let side_ok = o
            .stop
            .is_some_and(|stop| (reference - stop) * sign > Decimal::ZERO);
        let in_range = match (distance, atr[t]) {
            (Some(d), Some(a)) => {
                side_ok && d >= cfg.rules.stop_min_atr * a && d <= cfg.rules.stop_max_atr * a
            }
            _ => false,
        };
        let open_before = exits
            .iter()
            .any(|(decided, exit)| *decided <= t && *exit > t);
        let rules = vec![
            check(Rule::StopPresent, o.stop.is_some()),
            check(Rule::StopSide, side_ok),
            check(Rule::StopInRange, in_range),
            check(Rule::RiskWithinLimit, o.risk_pct <= cfg.rules.max_risk_pct),
            check(Rule::PlanWritten, !o.plan.trim().is_empty()),
            check(Rule::OnePosition, !open_before),
        ];
        // Sin stop, la pata corre hasta el final: un stop que nunca se toca.
        let leg_stop = o.stop.filter(|_| side_ok).unwrap_or(if side == Side::Long {
            Decimal::ZERO
        } else {
            Decimal::MAX
        });
        let entry = t + 1;
        let (exit_index, exit_mid, how) =
            simulate_leg(side, entry, leg_stop, o.target, last, bars, &[])
                .ok_or_else(|| ReplayError::Bad("sin velas para la entrada".into()))?;
        exits.push((t, exit_index));
        let entry_mid = bars[entry].open;
        let entry_price = fill_price(
            side,
            Execution::Entry,
            entry_mid,
            &costs.costs,
            &costs.execution,
        );
        let exit_price = fill_price(
            side,
            Execution::Exit,
            exit_mid,
            &costs.costs,
            &costs.execution,
        );
        let nights = funding_nights(
            bars[entry].start,
            bars[exit_index].end(),
            costs.funding_cutoff,
            costs.funding_tz,
        );
        let carry = funding(
            side,
            entry_mid,
            Decimal::ONE,
            nights,
            Decimal::ZERO,
            &costs.costs,
        );
        let per_unit_costs =
            (entry_price - entry_mid) * sign + (exit_mid - exit_price) * sign + carry;
        let r = |x: Decimal| {
            distance
                .filter(|d| *d > Decimal::ZERO && side_ok)
                .map(|d| (x / d).round_dp(4).normalize())
        };
        let r_gross = r((exit_mid - entry_mid) * sign);
        let costs_r = r(per_unit_costs);
        let outcome = Outcome {
            entry_index: entry,
            entry_price,
            exit_index,
            exit_price,
            exit: match how {
                StudyExit::Stop => ExitKind::Stop,
                StudyExit::StopGap => ExitKind::StopGap,
                StudyExit::Target => ExitKind::Target,
                StudyExit::Planned | StudyExit::ForceClose => ExitKind::End,
            },
            r_gross,
            costs_r,
            r_net: r_gross.zip(costs_r).map(|(g, c)| g - c),
        };
        trades.push(TradeGrade {
            order_id: o.order_id,
            rules,
            outcome: Some(outcome),
        });
    }
    let mut rules = Vec::new();
    let mut levels = None;
    match s.kind {
        ReplayKind::Trades => rules.push(check(
            Rule::EnoughTrades,
            s.orders.len() >= cfg.replay.trades_per_exam,
        )),
        ReplayKind::Levels => {
            let g = level_grade(s, cfg, &atr)?;
            let ok = g.precision_pct.is_some_and(|p| p >= cfg.pass_pct);
            rules.push(check(Rule::LevelPrecision, ok));
            levels = Some(g);
        }
        ReplayKind::Pivots | ReplayKind::Practice => {}
    }
    // Los pivotes se califican en su examen; en otra sesión con marcas de pivotes, solo se muestran.
    let pivot_marks = s.marks.iter().any(|m| m.kind != MarkKind::Level);
    let pivots = (s.kind == ReplayKind::Pivots || pivot_marks)
        .then(|| pivot_grade(s, cfg))
        .transpose()?;
    if let (ReplayKind::Pivots, Some(g)) = (s.kind, &pivots) {
        rules.push(check(
            Rule::EnoughPivots,
            g.marked >= cfg.replay.pivots_per_exam,
        ));
        let ok = g.precision_pct.is_some_and(|p| p >= cfg.pass_pct);
        rules.push(check(Rule::PivotPrecision, ok));
    }
    let all_ok = rules
        .iter()
        .chain(trades.iter().flat_map(|t| &t.rules))
        .all(|r| r.passed);
    Ok(ExamResult {
        session_id: s.id.clone(),
        kind: s.kind,
        lesson_id: s.lesson_id.clone(),
        catalog_version: cfg.catalog_version.to_owned(),
        passed: (s.kind != ReplayKind::Practice).then_some(all_ok),
        rules,
        trades,
        levels,
        pivots,
        revealed: Revealed {
            instrument_id: s.draw.instrument_id.clone(),
            timeframe: s.draw.timeframe,
            source: cfg.replay.source.clone(),
            first_bar_at: bars[0].start,
            last_bar_at: bars[last].start,
        },
        graded_at,
    })
}

/// Niveles marcados contra los del motor con las velas **hasta el cursor final** (lo que se veía).
fn level_grade(
    s: &ReplaySession,
    cfg: GradeConfig,
    atr: &[Option<Decimal>],
) -> Result<LevelGrade, ReplayError> {
    let c = s.cursor();
    let seen = &s.all_bars()?[..=c];
    let pivots = confirmed_pivots(seen, cfg.patterns.pivots.strength);
    let mut engine: Vec<Decimal> = levels_at(c, &pivots, &cfg.patterns.levels)
        .iter()
        .map(|l| l.price)
        .collect();
    engine.sort();
    let tolerance = cfg.replay.level_tolerance_atr * atr[c].unwrap_or_default();
    let marks: Vec<Decimal> = s
        .marks
        .iter()
        .filter(|m| m.kind == MarkKind::Level)
        .map(|m| m.price)
        .collect();
    let near = |a: Decimal, b: Decimal| (a - b).abs() <= tolerance;
    let matched_marks = marks
        .iter()
        .filter(|m| engine.iter().any(|e| near(**m, *e)))
        .count();
    let covered_levels = engine
        .iter()
        .filter(|e| marks.iter().any(|m| near(*m, **e)))
        .count();
    Ok(LevelGrade {
        marked: marks.len(),
        engine: engine.len(),
        matched_marks,
        covered_levels,
        precision_pct: pct(matched_marks, marks.len()),
        coverage_pct: pct(covered_levels, engine.len()),
        tolerance: tolerance.round_dp(6).normalize(),
        engine_levels: engine.iter().map(|p| p.round_dp(6).normalize()).collect(),
    })
}

/// Pivotes marcados contra los confirmados del motor con las velas **hasta el cursor final** (#800).
/// Un pivote que se confirmaría después del cursor no existe todavía: no cuenta. La cobertura mira
/// los que se confirmaron durante el examen (después del calentamiento).
fn pivot_grade(s: &ReplaySession, cfg: GradeConfig) -> Result<PivotGrade, ReplayError> {
    let seen = &s.all_bars()?[..=s.cursor()];
    let strength = cfg.patterns.pivots.strength;
    let all: Vec<EnginePivot> = confirmed_pivots(seen, strength)
        .iter()
        .map(|p| EnginePivot {
            bar_index: p.index,
            kind: match p.kind {
                PivotKind::High => MarkKind::PivotHigh,
                PivotKind::Low => MarkKind::PivotLow,
            },
            confirmed_at: p.confirmed_at,
        })
        .collect();
    // (marca, su calificación) de cada marca de pivote, en orden.
    let graded: Vec<(&crate::replay::Mark, PivotMarkGrade)> = s
        .marks
        .iter()
        .filter(|m| m.kind != MarkKind::Level)
        .map(|m| {
            let found = all
                .iter()
                .find(|p| Some(p.bar_index) == m.bar_index && p.kind == m.kind);
            let status = match found {
                Some(p) if m.cursor >= p.confirmed_at => PivotStatus::Hit,
                Some(_) => PivotStatus::Early,
                None => PivotStatus::Miss,
            };
            let g = PivotMarkGrade {
                mark_id: m.mark_id,
                status,
                confirmed_at: found.map(|p| p.confirmed_at),
            };
            (m, g)
        })
        .collect();
    let hit = |p: &EnginePivot| {
        graded.iter().any(|(m, g)| {
            g.status == PivotStatus::Hit && m.bar_index == Some(p.bar_index) && m.kind == p.kind
        })
    };
    let engine: Vec<EnginePivot> = all
        .into_iter()
        .filter(|p| p.confirmed_at >= s.warmup_bars)
        .collect();
    let covered = engine.iter().filter(|p| hit(p)).count();
    let marks: Vec<PivotMarkGrade> = graded.into_iter().map(|(_, g)| g).collect();
    let count = |st: PivotStatus| marks.iter().filter(|g| g.status == st).count();
    let (hits, early) = (count(PivotStatus::Hit), count(PivotStatus::Early));
    Ok(PivotGrade {
        strength,
        marked: marks.len(),
        hits,
        early,
        engine: engine.len(),
        covered,
        precision_pct: pct(hits, marks.len()),
        coverage_pct: pct(covered, engine.len()),
        marks,
        engine_pivots: engine,
    })
}
