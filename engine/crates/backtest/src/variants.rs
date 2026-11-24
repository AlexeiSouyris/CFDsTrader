//! Estudio de variantes en backtest (#359): **solo estudio**, nunca cambia la config.
//!
//! ```text
//! variante = nombre
//!          + overrides de strategy.toml / backtest.toml / risk.toml (claves que ya existen,
//!            mismo tipo: `override_toml`), aplicados en orden sobre el texto de config
//!          + horas UTC de la vela de entrada (t+1) en las que no se abre
//!          + conjuntos exactos de gatillos con los que no se abre
//! las dos últimas solo QUITAN marcas de entrada: la señal de la vela t se calcula igual que en
//! el recorrido (misma `Analysis` del tramo, mismas noticias) y, si se descarta, en t no se abre
//! tramos: las ventanas de prueba del walk-forward (`plan`) o, si los datos no alcanzan para una
//! ventana, bloques de `validation_months` antes del fuera de muestra (`research_blocks`)
//! ```
//! Ningún tramo llega al fuera de muestra (`research_period`). No elige ninguna variante.

use std::collections::BTreeSet;

use cfds_core::NewsInterpretation;
use cfds_market_data::{Period, SplitConfig};
use cfds_risk::RiskConfig;
use cfds_strategy::{Analysis, StrategyConfig, TriggerKind, news_adjustment};
use chrono::{DateTime, Datelike, Months, NaiveDate, Timelike, Utc};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    BacktestConfig, ConfigTexts, EvalError, ParamFile, RunError, Setup, Slice, TradeRecord, choose,
    override_toml,
};

/// Un parámetro de config pisado por la variante.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Override {
    pub file: ParamFile,
    pub key: String,
    pub value: toml::Value,
}

/// Una variante del estudio.
#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub name: String,
    pub overrides: Vec<Override>,
    /// Horas UTC (inicio de la vela de entrada, t+1) en las que no se abre.
    pub skip_entry_hours_utc: BTreeSet<u32>,
    /// Conjuntos exactos de gatillos con los que no se abre.
    pub skip_trigger_sets: Vec<BTreeSet<TriggerKind>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    variant: Vec<RawVariant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawVariant {
    name: String,
    #[serde(default)]
    overrides: Vec<Override>,
    #[serde(default)]
    skip_entry_hours_utc: Vec<u32>,
    #[serde(default)]
    skip_trigger_sets: Vec<Vec<String>>,
}

fn trigger(name: &str) -> Result<TriggerKind, EvalError> {
    Ok(match name {
        "candle_pattern" => TriggerKind::CandlePattern,
        "double_break" => TriggerKind::DoubleBreak,
        "level_rejection" => TriggerKind::LevelRejection,
        "sweep" => TriggerKind::Sweep,
        other => return Err(EvalError::Config(format!("gatillo desconocido: {other}"))),
    })
}

/// Lee las variantes (`[[variant]]`). Nombres únicos, horas de 0 a 23, gatillos conocidos.
pub fn variants_from_toml(text: &str) -> Result<Vec<Variant>, EvalError> {
    let raw: RawFile = toml::from_str(text).map_err(|e| EvalError::Config(e.to_string()))?;
    let mut names = BTreeSet::new();
    raw.variant
        .into_iter()
        .map(|v| {
            if !names.insert(v.name.clone()) {
                return Err(EvalError::Config(format!("variante repetida: {}", v.name)));
            }
            if let Some(h) = v.skip_entry_hours_utc.iter().find(|h| **h > 23) {
                return Err(EvalError::Config(format!(
                    "{}: hora {h} fuera de 0–23",
                    v.name
                )));
            }
            let sets = v
                .skip_trigger_sets
                .iter()
                .map(|set| {
                    if set.is_empty() {
                        return Err(EvalError::Config(format!(
                            "{}: conjunto de gatillos vacío",
                            v.name
                        )));
                    }
                    set.iter().map(|s| trigger(s)).collect()
                })
                .collect::<Result<_, _>>()?;
            Ok(Variant {
                name: v.name,
                overrides: v.overrides,
                skip_entry_hours_utc: v.skip_entry_hours_utc.into_iter().collect(),
                skip_trigger_sets: sets,
            })
        })
        .collect()
}

/// `base` con los overrides de la variante, en orden y acumulados por archivo. Lo demás queda
/// igual (también el reinicio del kill switch que la corrida pisó).
pub fn setup_for(base: &Setup, texts: &ConfigTexts, v: &Variant) -> Result<Setup, EvalError> {
    let (mut strategy, mut backtest, mut risk) = (None::<String>, None::<String>, None::<String>);
    for o in &v.overrides {
        let (slot, original) = match o.file {
            ParamFile::Strategy => (&mut strategy, texts.strategy),
            ParamFile::Backtest => (&mut backtest, texts.backtest),
            ParamFile::Risk => (&mut risk, texts.risk),
        };
        let current = slot.as_deref().unwrap_or(original);
        *slot = Some(override_toml(current, &o.key, &o.value)?);
    }
    let bad = |f: &str, e: String| EvalError::Config(format!("{}: {f}: {e}", v.name));
    let mut s = base.clone();
    if let Some(t) = strategy {
        s.strategy.strategy =
            StrategyConfig::from_toml(&t).map_err(|e| bad("strategy.toml", e.to_string()))?;
    }
    if let Some(t) = backtest {
        let mut b =
            BacktestConfig::from_toml(&t).map_err(|e| bad("backtest.toml", e.to_string()))?;
        b.kill_switch_reset_days = base.backtest.kill_switch_reset_days;
        s.backtest = b;
    }
    if let Some(t) = risk {
        s.risk = RiskConfig::from_toml(&t).map_err(|e| bad("risk.toml", e.to_string()))?;
    }
    Ok(s)
}

/// Marcas de entrada de la variante para un tramo (alineadas con `slice.m15`): `false` donde
/// la vela de entrada t+1 empieza en una hora excluida o donde la señal de t tiene un conjunto
/// de gatillos excluido. La señal es la misma que elegiría el recorrido: misma `Analysis` del
/// tramo y mismo ajuste de noticias (solo lo publicado desde `published_from` y visto al cierre).
pub fn entry_filter(
    slice: &Slice,
    entries: &[bool],
    setup: &Setup,
    v: &Variant,
) -> Result<Vec<bool>, RunError> {
    let m15 = slice.m15;
    let mut out = entries.to_vec();
    if !v.skip_entry_hours_utc.is_empty() {
        for (t, e) in out.iter_mut().enumerate() {
            match m15.get(t + 1) {
                Some(next) if v.skip_entry_hours_utc.contains(&next.start.hour()) => *e = false,
                _ => {}
            }
        }
    }
    if v.skip_trigger_sets.is_empty() {
        return Ok(out);
    }
    let news: Vec<NewsInterpretation> = slice
        .news
        .iter()
        .filter(|n| n.published_at >= slice.published_from)
        .cloned()
        .collect();
    let analysis =
        Analysis::new(m15, slice.h1, setup.strategy).with_data_errors(slice.data_errors.to_vec());
    for t in 0..out.len() {
        if !out[t] {
            continue;
        }
        let at = m15[t].end();
        let adj = |side| {
            news_adjustment(
                &news,
                &setup.instrument,
                side,
                at,
                &setup.strategy.strategy.news,
                &setup.strategy.strategy.score,
            )
        };
        if let Some(signal) = choose(&analysis, t, adj).map_err(RunError::Score)? {
            let set: BTreeSet<TriggerKind> = signal.triggers.iter().map(|g| g.kind).collect();
            if v.skip_trigger_sets.contains(&set) {
                out[t] = false;
            }
        }
    }
    Ok(out)
}

fn month_start(d: NaiveDate) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1)
}

fn midnight(d: NaiveDate) -> DateTime<Utc> {
    d.and_time(chrono::NaiveTime::MIN).and_utc()
}

/// Bloques de investigación cuando los datos no alcanzan para una ventana del walk-forward:
/// desde el primer mes completo, bloques de `validation_months` que avanzan `step_months`,
/// todos terminados antes del fuera de muestra, que se calcula igual que en `plan` (los últimos
/// `out_of_sample_months` contados desde el mes de la última vela). Vacío si no entra ninguno.
pub fn research_blocks(
    first: DateTime<Utc>,
    last: DateTime<Utc>,
    cfg: &SplitConfig,
) -> (Vec<Period>, Option<DateTime<Utc>>) {
    let Some(oos) = month_start(last.date_naive())
        .and_then(|m| m.checked_sub_months(Months::new(cfg.out_of_sample_months)))
    else {
        return (Vec::new(), None);
    };
    let first_day = first.date_naive();
    let start = if first_day.day() == 1 && first.time() == chrono::NaiveTime::MIN {
        Some(first_day)
    } else {
        month_start(first_day).and_then(|m| m.checked_add_months(Months::new(1)))
    };
    let mut blocks = Vec::new();
    let mut s = start;
    while let Some(b) = s {
        let Some(e) = b
            .checked_add_months(Months::new(cfg.validation_months))
            .filter(|e| *e <= oos)
        else {
            break;
        };
        blocks.push(Period {
            start: midnight(b),
            end: midnight(e),
        });
        s = b.checked_add_months(Months::new(cfg.step_months.max(1)));
    }
    (blocks, Some(midnight(oos)))
}

/// Agregados de operaciones de un tramo, en la moneda de la cuenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TradeStats {
    pub trades: usize,
    /// Operaciones con neto positivo.
    pub wins: usize,
    pub losses: usize,
    /// Suma del neto de las ganadoras y de las perdedoras (esta, negativa o cero).
    pub won: Decimal,
    pub lost: Decimal,
    /// P/L a precio medio, sin ningún costo.
    pub mid_pnl: Decimal,
    /// Spread, slippage, comisión y financiación: `mid_pnl − net`.
    pub costs: Decimal,
    pub net: Decimal,
}

impl TradeStats {
    pub fn of(trades: &[TradeRecord], account_to_instrument: Decimal) -> Self {
        let mut s = Self::default();
        for t in trades {
            let net = t.net_pnl / account_to_instrument;
            let mid = t.mid_pnl / account_to_instrument;
            s.trades += 1;
            s.net += net;
            s.mid_pnl += mid;
            if net > Decimal::ZERO {
                s.wins += 1;
                s.won += net;
            } else if net < Decimal::ZERO {
                s.losses += 1;
                s.lost += net;
            }
        }
        s.costs = s.mid_pnl - s.net;
        s
    }

    pub fn add(&mut self, o: &Self) {
        self.trades += o.trades;
        self.wins += o.wins;
        self.losses += o.losses;
        self.won += o.won;
        self.lost += o.lost;
        self.mid_pnl += o.mid_pnl;
        self.costs += o.costs;
        self.net += o.net;
    }

    pub fn win_rate(&self) -> Option<Decimal> {
        (self.trades > 0).then(|| Decimal::from(self.wins) / Decimal::from(self.trades))
    }

    /// Ganancia media de las ganadoras / pérdida media de las perdedoras.
    pub fn payoff(&self) -> Option<Decimal> {
        if self.wins == 0 || self.losses == 0 {
            return None;
        }
        let avg_win = self.won / Decimal::from(self.wins);
        let avg_loss = self.lost / Decimal::from(self.losses);
        Some(avg_win / avg_loss.abs())
    }
}
