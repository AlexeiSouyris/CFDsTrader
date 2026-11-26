//! Los bloques que el motor sabe correr (tipos y nombres de parámetros, en el código) y el
//! catálogo que arma con los números de `config/lab.toml` y `config/risk.toml`.

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, Timeframe};
use cfds_risk::RiskConfig;
use rust_decimal::Decimal;
use serde::Deserialize;

use super::catalog::{
    BlockSchema, Limits, MarketDef, ParamKind, ParamSchema, Slot, StrategyCatalog, parse_time,
};
use super::spec::{Mode, Source};

#[derive(Clone, Copy)]
enum Kind {
    Integer,
    Decimal,
    Time,
    Enum(&'static [&'static str]),
    Boolean,
}

/// `(nombre, etiqueta, tipo, obligatorio)`.
type ParamDef = (&'static str, &'static str, Kind, bool);

struct BlockDef {
    slot: Slot,
    kind: &'static str,
    label: &'static str,
    description: &'static str,
    timeframes: &'static [Timeframe],
    params: &'static [ParamDef],
}

const BOTH: &[Timeframe] = &[Timeframe::M15, Timeframe::H1];

const BLOCKS: &[BlockDef] = &[
    BlockDef {
        slot: Slot::Entry,
        kind: "ma_cross",
        label: "Cruce de dos medias",
        description: "Media rápida y lenta de los cierres (SMA o EMA) del timeframe de la señal. \
            Largo al cierre de la vela en que la rápida queda arriba de la lenta y antes estaba \
            abajo; corto en el espejo. Sin las dos medias (calentamiento) no hay señal.",
        timeframes: BOTH,
        params: &[
            ("average", "Media", Kind::Enum(&["sma", "ema"]), true),
            ("fast", "Media rápida (velas)", Kind::Integer, true),
            ("slow", "Media lenta (velas)", Kind::Integer, true),
        ],
    },
    BlockDef {
        slot: Slot::Entry,
        kind: "opening_range_breakout",
        label: "Ruptura del rango de apertura",
        description: "Rango = máximo y mínimo de las primeras velas de 15m de la sesión de contado; \
            largo si una vela posterior cierra arriba del máximo, corto si cierra abajo del mínimo; \
            una sola ruptura por día y solo hasta la hora local indicada (familia C de la ronda 1; \
            Crabel, 1990).",
        timeframes: &[Timeframe::M15],
        params: &[
            ("range_candles", "Velas del rango", Kind::Integer, true),
            (
                "last_breakout_close",
                "Último cierre de la ruptura (hora local)",
                Kind::Time,
                true,
            ),
        ],
    },
    BlockDef {
        slot: Slot::Filter,
        kind: "entry_hours_utc",
        label: "Horas de entrada (UTC)",
        description: "Solo deja entrar si la decisión (cierre de la vela de la señal) cae en \
            [desde, hasta) UTC. Si desde > hasta, el tramo cruza la medianoche.",
        timeframes: BOTH,
        params: &[
            ("from", "Desde (UTC)", Kind::Time, true),
            ("to", "Hasta (UTC)", Kind::Time, true),
        ],
    },
    BlockDef {
        slot: Slot::Stop,
        kind: "atr",
        label: "Stop en ATR",
        description: "Stop a k × ATR de Wilder del timeframe de la señal desde el cierre de la vela \
            de la señal. Ese mismo ATR es el que usa el módulo de riesgo.",
        timeframes: BOTH,
        params: &[
            ("period", "Período del ATR (velas)", Kind::Integer, true),
            ("k", "Múltiplo del ATR", Kind::Decimal, true),
        ],
    },
    BlockDef {
        slot: Slot::Exit,
        kind: "time_target",
        label: "Tiempo y objetivo",
        description: "Sale al cierre de la vela N del timeframe de la señal y/o al cierre de la \
            sesión de contado (lo que llegue antes; hace falta al menos uno), con objetivo \
            opcional en múltiplos de R. Los cierres forzados (fin de semana, vencimiento) siempre \
            se aplican.",
        timeframes: BOTH,
        params: &[
            ("max_candles", "Velas hasta la salida", Kind::Integer, false),
            (
                "session_close",
                "Salir al cierre de la sesión",
                Kind::Boolean,
                false,
            ),
            ("target_r", "Objetivo (R)", Kind::Decimal, false),
        ],
    },
];

// ── config/lab.toml ────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamRange {
    pub min: Option<String>,
    pub max: Option<String>,
    pub default: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabLimits {
    pub max_entry_conditions: usize,
    pub max_filters: usize,
    pub max_cost_multiplier: Decimal,
    pub max_variants_per_run: usize,
}

/// `config/lab.toml`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabConfig {
    pub catalog_version: u32,
    pub limits: LabLimits,
    pub params: BTreeMap<String, BTreeMap<String, ParamRange>>,
    pub report: super::sections::ReportConfig,
    pub worker: super::worker::WorkerConfig,
}

impl LabConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let cfg: Self = toml::from_str(text).map_err(|e| format!("lab.toml: {e}"))?;
        cfg.worker.validate()?;
        Ok(cfg)
    }
}

fn parse<T: std::str::FromStr>(v: Option<&String>, what: &str) -> Result<T, String> {
    v.ok_or_else(|| format!("lab.toml: falta {what}"))?
        .parse()
        .map_err(|_| format!("lab.toml: {what} no tiene el tipo esperado"))
}

fn param_schema(
    block: &str,
    def: &ParamDef,
    r: &ParamRange,
    risk: &RiskConfig,
) -> Result<ParamSchema, String> {
    let (name, label, kind, required) = *def;
    let at = |k: &str| format!("params.{block}.{name}.{k}");
    let default = r.default.as_ref();
    let bad = || format!("lab.toml: params.{block}.{name}: rango, default o tipo inválido");
    let kind = match kind {
        Kind::Integer => {
            let (min, max): (i64, i64) = (
                parse(r.min.as_ref(), &at("min"))?,
                parse(r.max.as_ref(), &at("max"))?,
            );
            let default = default
                .map(|_| parse::<i64>(default, &at("default")))
                .transpose()?;
            (min <= max && default.is_none_or(|d| (min..=max).contains(&d)))
                .then_some(ParamKind::Integer { min, max, default })
        }
        Kind::Decimal => {
            // La distancia del stop en ATR es la de config/risk.toml: el riesgo la exige igual.
            let (min, max): (Decimal, Decimal) = if (block, name) == ("atr", "k") {
                (risk.min_stop_atr, risk.max_stop_atr)
            } else {
                (
                    parse(r.min.as_ref(), &at("min"))?,
                    parse(r.max.as_ref(), &at("max"))?,
                )
            };
            let default = default
                .map(|_| parse::<Decimal>(default, &at("default")))
                .transpose()?;
            let ok = Decimal::ZERO <= min
                && min <= max
                && default.is_none_or(|d| (min..=max).contains(&d));
            ok.then_some(ParamKind::Decimal { min, max, default })
        }
        Kind::Time => default
            .is_none_or(|d| parse_time(d).is_some())
            .then(|| ParamKind::Time {
                default: default.cloned(),
            }),
        Kind::Enum(options) => default
            .is_none_or(|d| options.contains(&d.as_str()))
            .then(|| ParamKind::Enum {
                options: options.iter().map(|o| (*o).to_owned()).collect(),
                default: default.cloned(),
            }),
        Kind::Boolean => {
            let default = default
                .map(|_| parse::<bool>(default, &at("default")))
                .transpose()?;
            Some(ParamKind::Boolean { default })
        }
    };
    let ranged = matches!(
        kind,
        Some(ParamKind::Integer { .. } | ParamKind::Decimal { .. })
    );
    match kind {
        Some(kind) if ranged || (r.min.is_none() && r.max.is_none()) => Ok(ParamSchema {
            name: name.to_owned(),
            label: label.to_owned(),
            required,
            kind,
        }),
        _ => Err(bad()),
    }
}

/// El catálogo del motor: los bloques del código con los rangos de `config/lab.toml`, el riesgo
/// máximo y la distancia del stop de `config/risk.toml`, y los `instruments` con velas de
/// futuros. Error si a config le falta un parámetro, tiene uno que el código no conoce o un
/// rango inválido (se valida al arrancar).
pub fn catalog(
    cfg: &LabConfig,
    risk: &RiskConfig,
    instruments: impl IntoIterator<Item = InstrumentId>,
) -> Result<StrategyCatalog, String> {
    for (block, params) in &cfg.params {
        let def = BLOCKS
            .iter()
            .find(|b| b.kind == block)
            .ok_or_else(|| format!("lab.toml: bloque desconocido params.{block}"))?;
        if let Some(p) = params
            .keys()
            .find(|p| !def.params.iter().any(|d| d.0 == p.as_str()))
        {
            return Err(format!(
                "lab.toml: parámetro desconocido params.{block}.{p}"
            ));
        }
    }
    let mut blocks = Vec::new();
    for b in BLOCKS {
        let ranges = cfg.params.get(b.kind);
        let params = b
            .params
            .iter()
            .map(|d| {
                let r = ranges
                    .and_then(|r| r.get(d.0))
                    .ok_or_else(|| format!("lab.toml: falta params.{}.{}", b.kind, d.0))?;
                param_schema(b.kind, d, r, risk)
            })
            .collect::<Result<_, String>>()?;
        blocks.push(BlockSchema {
            slot: b.slot,
            kind: b.kind.to_owned(),
            label: b.label.to_owned(),
            description: b
                .description
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" "),
            timeframes: b.timeframes.to_vec(),
            params,
        });
    }
    let l = &cfg.limits;
    let max_cost_multiplier = l.max_cost_multiplier;
    if cfg.catalog_version == 0
        || l.max_entry_conditions == 0
        || l.max_variants_per_run == 0
        || max_cost_multiplier < Decimal::ONE
    {
        return Err(
            "lab.toml: catalog_version, max_entry_conditions y max_variants_per_run \
            tienen que ser > 0 y max_cost_multiplier ≥ 1"
                .into(),
        );
    }
    Ok(StrategyCatalog {
        catalog_version: cfg.catalog_version,
        limits: Limits {
            max_entry_conditions: l.max_entry_conditions,
            max_filters: l.max_filters,
            max_risk_pct: risk.risk_per_trade_pct,
            min_cost_multiplier: Decimal::ONE,
            max_cost_multiplier,
            max_variants_per_run: l.max_variants_per_run,
        },
        markets: instruments
            .into_iter()
            .map(|instrument_id| MarketDef {
                instrument_id,
                source: Source::Futures,
                timeframes: BOTH.to_vec(),
            })
            .collect(),
        // El confirmatorio llega con el flujo de pre-registro (L13).
        modes: vec![Mode::Exploratory],
        blocks,
    })
}
