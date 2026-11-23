//! Informe de aprendizaje continuo (fase 8b, spec §15b, #327): `learning_report.v1`.
//!
//! ```text
//! diario (proposal.v1 + outcome.v1) del período, de paper o de corridas de backtest
//!   1. calibración: probabilidad mostrada (§13, con la tabla de entrenamiento) por rango
//!      → frecuencia real de TP1 antes que el stop, con su intervalo de Wilson
//!      → con menos de min_cases casos: "datos insuficientes"; muestra para verificar a mano
//!   2. aporte por señal: R medio y % de TP1 con la señal y sin ella (muestra chica marcada)
//!   3. propuestas de cambio: texto que cita la evidencia, estado "pendiente de voto"
//! ```
//! **El sistema nunca se cambia solo** (spec §15b): el informe solo lee y propone. Cada propuesta
//! pasa por votación (spec §1.2); nada de acá toca `config/`. Funciones puras.

use cfds_core::candle::utc_z;
use cfds_core::decimal_str;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::probability::{ProbabilityConfig, ProbabilityTable, Shown, wilson};
use crate::{Outcome, Proposal, Signal, Source};

/// Todas las señales del desglose (spec §6), en el orden del informe.
pub const SIGNALS: [Signal; 9] = [
    Signal::Trend,
    Signal::SmaSide,
    Signal::CandlePattern,
    Signal::DoubleBreak,
    Signal::LevelRejection,
    Signal::Sweep,
    Signal::FvgOrOrderBlock,
    Signal::RecentChoch,
    Signal::News,
];

/// `config/learning.toml`, validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearningConfig {
    /// Límites de los rangos de probabilidad, de 0 a 1, estrictamente crecientes.
    pub buckets: Vec<Decimal>,
    pub sample_per_bucket: usize,
    pub signal_min_cases: u32,
}

impl LearningConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            calibration: RawCal,
            signals: RawSig,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawCal {
            buckets: Vec<String>,
            sample_per_bucket: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawSig {
            min_cases: u32,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| format!("config/learning.toml: {e}"))?;
        let buckets = raw
            .calibration
            .buckets
            .iter()
            .map(|b| b.parse::<Decimal>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "calibration.buckets: decimales como texto".to_string())?;
        let ok = buckets.len() >= 2
            && buckets.first() == Some(&Decimal::ZERO)
            && buckets.last() == Some(&Decimal::ONE)
            && buckets.windows(2).all(|w| w[0] < w[1]);
        if !ok {
            return Err("calibration.buckets: de 0 a 1, estrictamente crecientes".into());
        }
        if raw.calibration.sample_per_bucket == 0 || raw.signals.min_cases == 0 {
            return Err("sample_per_bucket y signals.min_cases > 0".into());
        }
        Ok(Self {
            buckets,
            sample_per_bucket: raw.calibration.sample_per_bucket,
            signal_min_cases: raw.signals.min_cases,
        })
    }

    /// Rango de una probabilidad: `[b_i, b_i+1)`, y el último incluye el 1.
    fn bucket(&self, p: Decimal) -> Option<usize> {
        let last = self.buckets.len() - 2;
        if p == Decimal::ONE {
            return Some(last);
        }
        self.buckets.windows(2).position(|w| p >= w[0] && p < w[1])
    }
}

/// Una fila de la tabla de confiabilidad.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CalibrationRow {
    #[serde(with = "decimal_str")]
    pub from: Decimal,
    #[serde(with = "decimal_str")]
    pub to: Decimal,
    pub cases: u32,
    pub hits: u32,
    /// Probabilidad mostrada promedio de las propuestas del rango.
    #[serde(with = "decimal_str::option")]
    pub mean_shown: Option<Decimal>,
    /// Frecuencia real de "tocó TP1 antes que el stop".
    #[serde(with = "decimal_str::option")]
    pub frequency: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub low: Option<Decimal>,
    #[serde(with = "decimal_str::option")]
    pub high: Option<Decimal>,
    /// Menos de `min_cases` casos (spec §15b): no se saca conclusión.
    pub insufficient: bool,
}

/// Una propuesta de la muestra, para verificar la calibración a mano.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SampleItem {
    pub proposal_id: String,
    #[serde(with = "utc_z")]
    pub signal_at: DateTime<Utc>,
    #[serde(with = "decimal_str")]
    pub shown: Decimal,
    pub hit_tp1: bool,
    /// Índice de la fila de la tabla.
    pub bucket: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Calibration {
    pub rows: Vec<CalibrationRow>,
    /// Propuestas sin probabilidad mostrada (§13: "datos insuficientes" en su grupo).
    pub without_probability: u32,
    pub sample: Vec<SampleItem>,
}

/// Resultado de un grupo de propuestas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stats {
    pub cases: u32,
    pub hits: u32,
    #[serde(with = "decimal_str::option")]
    pub hit_rate: Option<Decimal>,
    /// P/L neto en R (R = |referencia − stop| × tamaño), promedio.
    #[serde(with = "decimal_str::signed_option")]
    pub mean_r: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SignalRow {
    pub signal: Signal,
    pub with: Stats,
    pub without: Stats,
    /// R medio con la señal − sin ella.
    #[serde(with = "decimal_str::signed_option")]
    pub difference_r: Option<Decimal>,
    /// Menos de `signals.min_cases` casos con o sin la señal: no genera propuesta.
    pub small_sample: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Recalibrar la probabilidad mostrada de un rango.
    Recalibrate,
    /// Revisar el peso de una señal (bajarlo o sacarla).
    ReviewSignalWeight,
}

/// Una propuesta de cambio del informe. Nunca se aplica sola: queda pendiente de voto.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeProposal {
    pub change_id: String,
    pub kind: ChangeKind,
    pub text: String,
    /// Dónde está la evidencia en el informe (por ejemplo `calibration.rows[5]`).
    pub evidence: String,
    /// Siempre `pendiente_de_voto` al crearse (spec §1.2).
    pub status: &'static str,
}

/// `learning_report.v1`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearningReport {
    pub report_id: String,
    #[serde(with = "utc_z")]
    pub created_at: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub from: DateTime<Utc>,
    #[serde(with = "utc_z")]
    pub to: DateTime<Utc>,
    pub sources: Vec<Source>,
    /// Propuestas con resultado evaluadas.
    pub proposals: u32,
    pub calibration: Calibration,
    pub signals: Vec<SignalRow>,
    pub change_proposals: Vec<ChangeProposal>,
}

pub const PENDING_VOTE: &str = "pendiente_de_voto";

fn ratio(a: u32, b: u32) -> Option<Decimal> {
    (b > 0).then(|| (Decimal::from(a) / Decimal::from(b)).round_dp(4))
}

fn has(p: &Proposal, s: Signal) -> bool {
    p.triggers.contains(&s)
        || p.breakdown
            .iter()
            .any(|b| b.component == s && b.points != 0)
}

/// P/L neto en R de una propuesta, o `None` si su riesgo es cero.
pub fn r_multiple(p: &Proposal, o: &Outcome) -> Option<Decimal> {
    let risk = (p.reference - p.stop).abs() * p.size;
    (!risk.is_zero()).then(|| o.net_pnl / risk)
}

fn stats<'a>(pairs: impl Iterator<Item = &'a (Proposal, Outcome)>) -> Stats {
    let (mut cases, mut hits, mut sum_r, mut with_r) = (0u32, 0u32, Decimal::ZERO, 0u32);
    for (p, o) in pairs {
        cases += 1;
        hits += u32::from(o.hit_tp1);
        if let Some(r) = r_multiple(p, o) {
            sum_r += r;
            with_r += 1;
        }
    }
    Stats {
        cases,
        hits,
        hit_rate: ratio(hits, cases),
        mean_r: (with_r > 0).then(|| (sum_r / Decimal::from(with_r)).round_dp(4)),
    }
}

/// Lo que entra al informe.
pub struct Inputs<'a> {
    /// Pares propuesta + resultado del período (ya filtrados por quien llama).
    pub pairs: &'a [(Proposal, Outcome)],
    /// La tabla de la probabilidad mostrada (§13), armada con el backtest de entrenamiento.
    pub table: &'a ProbabilityTable,
    pub probability: &'a ProbabilityConfig,
    pub cfg: &'a LearningConfig,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

/// Arma el informe. Puro: misma entrada, misma salida.
pub fn report(i: &Inputs<'_>) -> LearningReport {
    let cfg = i.cfg;
    let min_cases = i.probability.min_cases;
    let report_id = format!(
        "learning:{}:{}:{}",
        i.from.format("%Y%m%dT%H%MZ"),
        i.to.format("%Y%m%dT%H%MZ"),
        i.created_at.format("%Y%m%dT%H%M%SZ")
    );
    let mut sorted: Vec<&(Proposal, Outcome)> = i.pairs.iter().collect();
    sorted.sort_by(|a, b| (a.0.signal_at, &a.0.id).cmp(&(b.0.signal_at, &b.0.id)));

    // 1. Calibración.
    let n_buckets = cfg.buckets.len() - 1;
    let mut acc: Vec<(u32, u32, Decimal)> = vec![(0, 0, Decimal::ZERO); n_buckets];
    let mut sample = Vec::new();
    let mut taken = vec![0usize; n_buckets];
    let mut without_probability = 0;
    for (p, o) in &sorted {
        let Shown::Estimate { probability, .. } = i.table.shown(p) else {
            without_probability += 1;
            continue;
        };
        let Some(b) = cfg.bucket(probability) else {
            without_probability += 1;
            continue;
        };
        acc[b].0 += 1;
        acc[b].1 += u32::from(o.hit_tp1);
        acc[b].2 += probability;
        if taken[b] < cfg.sample_per_bucket {
            taken[b] += 1;
            sample.push(SampleItem {
                proposal_id: p.id.clone(),
                signal_at: p.signal_at,
                shown: probability,
                hit_tp1: o.hit_tp1,
                bucket: b,
            });
        }
    }
    let rows: Vec<CalibrationRow> = acc
        .iter()
        .enumerate()
        .map(|(b, &(cases, hits, sum))| {
            let (low, high) = if cases > 0 {
                let (l, h) = wilson(hits, cases, i.probability.z);
                (Some(l.round_dp(4)), Some(h.round_dp(4)))
            } else {
                (None, None)
            };
            CalibrationRow {
                from: cfg.buckets[b],
                to: cfg.buckets[b + 1],
                cases,
                hits,
                mean_shown: (cases > 0).then(|| (sum / Decimal::from(cases)).round_dp(4)),
                frequency: ratio(hits, cases),
                low,
                high,
                insufficient: cases < min_cases,
            }
        })
        .collect();

    // 2. Aporte por señal.
    let signals: Vec<SignalRow> = SIGNALS
        .iter()
        .map(|&s| {
            let with = stats(sorted.iter().copied().filter(|(p, _)| has(p, s)));
            let without = stats(sorted.iter().copied().filter(|(p, _)| !has(p, s)));
            let difference_r = match (with.mean_r, without.mean_r) {
                (Some(a), Some(b)) => Some(a - b),
                _ => None,
            };
            let small_sample =
                with.cases < cfg.signal_min_cases || without.cases < cfg.signal_min_cases;
            SignalRow {
                signal: s,
                with,
                without,
                difference_r,
                small_sample,
            }
        })
        .collect();

    // 3. Propuestas de cambio (texto con su evidencia; nunca se aplican solas).
    let pct = |d: Decimal| format!("{}%", (d * Decimal::ONE_HUNDRED).round_dp(1));
    let mut change_proposals = Vec::new();
    for (b, r) in rows.iter().enumerate() {
        if r.insufficient {
            continue;
        }
        let (Some(m), Some(f), Some(lo), Some(hi)) = (r.mean_shown, r.frequency, r.low, r.high)
        else {
            continue;
        };
        if m < lo || m > hi {
            change_proposals.push(ChangeProposal {
                change_id: format!("{report_id}:recalibrate:{b}"),
                kind: ChangeKind::Recalibrate,
                text: format!(
                    "Recalibrar la probabilidad mostrada en el rango {}–{}: se mostró en promedio \
                     {} y pasó {} (intervalo {}–{}, {} casos). Es una propuesta para votar; no \
                     cambia nada sola.",
                    pct(r.from),
                    pct(r.to),
                    pct(m),
                    pct(f),
                    pct(lo),
                    pct(hi),
                    r.cases
                ),
                evidence: format!("calibration.rows[{b}]"),
                status: PENDING_VOTE,
            });
        }
    }
    for (k, s) in signals.iter().enumerate() {
        let (Some(d), Some(a), Some(b)) = (s.difference_r, s.with.mean_r, s.without.mean_r) else {
            continue;
        };
        if s.small_sample || d >= Decimal::ZERO {
            continue;
        }
        let name = serde_json::to_value(s.signal)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_default();
        change_proposals.push(ChangeProposal {
            change_id: format!("{report_id}:signal:{name}"),
            kind: ChangeKind::ReviewSignalWeight,
            text: format!(
                "Revisar el peso de la señal {name}: con ella el resultado medio fue {a} R ({} \
                 casos) y sin ella {b} R ({} casos). Es una propuesta para votar (bajar el peso o \
                 sacarla); no cambia nada sola.",
                s.with.cases, s.without.cases
            ),
            evidence: format!("signals[{k}]"),
            status: PENDING_VOTE,
        });
    }

    let mut sources: Vec<Source> = Vec::new();
    for (p, _) in &sorted {
        if !sources.contains(&p.source) {
            sources.push(p.source);
        }
    }
    LearningReport {
        report_id,
        created_at: i.created_at,
        from: i.from,
        to: i.to,
        sources,
        proposals: u32::try_from(sorted.len()).unwrap_or(u32::MAX),
        calibration: Calibration {
            rows,
            without_probability,
            sample,
        },
        signals,
        change_proposals,
    }
}
