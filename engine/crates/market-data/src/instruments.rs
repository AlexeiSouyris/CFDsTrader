//! Catálogo de instrumentos y condiciones del broker: `config/instruments.toml` (#7, ADR 0002).
//!
//! Cada parámetro del broker es [`Sourced`]: o tiene valor **y fuente**, o está **pendiente** con
//! su motivo. Nunca hay un número sin origen (CLAUDE.md §5b). Mientras falte un costo,
//! [`Catalog::ready_for_backtest`] devuelve error (spec §11).

use std::collections::{BTreeMap, HashMap};
use std::str::FromStr;

use cfds_core::{AssetClass, Currency, Instrument, InstrumentId};
use chrono_tz::Tz;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CatalogError {
    #[error("config/instruments.toml no es TOML válido: {0}")]
    Parse(String),
    #[error("config/instruments.toml, {item}.{field}: {problem}")]
    Invalid {
        item: String,
        field: String,
        problem: String,
    },
}

/// Un parámetro con su origen, o pendiente de confirmar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sourced<T> {
    Known { value: T, source: String },
    Pending { reason: String },
}

impl<T> Sourced<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Sourced::Known { value, .. } => Some(value),
            Sourced::Pending { .. } => None,
        }
    }
}

/// Condiciones de IG para un instrumento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokerTerms {
    pub epic: Sourced<String>,
    pub margin_retail_pct: Sourced<Decimal>,
    pub spread_min_points: Sourced<Decimal>,
    pub spread_typical_points: Sourced<Decimal>,
    pub value_per_point: Sourced<Decimal>,
    pub min_deal_size: Sourced<Decimal>,
    /// Movimiento mínimo de precio, en puntos.
    pub tick: Sourced<Decimal>,
    pub funding_admin_fee_pct: Sourced<Decimal>,
    pub funding_day_count: Sourced<Decimal>,
    pub funding_benchmark: Sourced<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentSpec {
    pub instrument: Instrument,
    /// Bolsa del subyacente, para el calendario de sesiones (#9).
    pub exchange: String,
    /// Tope propio de apalancamiento (spec §8). Nunca supera el regulatorio (100 / margen).
    pub max_leverage: Decimal,
    pub terms: BrokerTerms,
}

impl InstrumentSpec {
    /// Campos pendientes de este instrumento.
    pub fn pending(&self) -> Vec<PendingField> {
        let item = self.instrument.id.as_str();
        let t = &self.terms;
        let mut out = Vec::new();
        let mut add = |field: &'static str, reason: Option<&String>| {
            if let Some(r) = reason {
                out.push(PendingField {
                    item: item.to_owned(),
                    field,
                    reason: r.clone(),
                });
            }
        };
        let text = |p: &Sourced<String>| match p {
            Sourced::Pending { reason } => Some(reason.clone()),
            Sourced::Known { .. } => None,
        };
        let num = |p: &Sourced<Decimal>| match p {
            Sourced::Pending { reason } => Some(reason.clone()),
            Sourced::Known { .. } => None,
        };
        add("epic", text(&t.epic).as_ref());
        add("funding_benchmark", text(&t.funding_benchmark).as_ref());
        for (field, v) in [
            ("margin_retail_pct", &t.margin_retail_pct),
            ("spread_min_points", &t.spread_min_points),
            ("spread_typical_points", &t.spread_typical_points),
            ("value_per_point", &t.value_per_point),
            ("min_deal_size", &t.min_deal_size),
            ("tick", &t.tick),
            ("funding_admin_fee_pct", &t.funding_admin_fee_pct),
            ("funding_day_count", &t.funding_day_count),
        ] {
            add(field, num(v).as_ref());
        }
        out
    }

    /// Costos que faltan para simular **este** instrumento (spec §11). El `epic` no cuenta: solo
    /// hace falta para conectarse en vivo.
    pub fn missing_costs(&self) -> Vec<PendingField> {
        self.pending()
            .into_iter()
            .filter(|p| p.field != "epic")
            .collect()
    }
}

/// Un campo pendiente: qué instrumento, qué campo y por qué falta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingField {
    pub item: String,
    pub field: &'static str,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub instruments: BTreeMap<InstrumentId, InstrumentSpec>,
    pub margin_closeout_pct: Sourced<Decimal>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourced {
    value: Option<String>,
    source: Option<String>,
    pending: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    #[allow(dead_code)] // se exige para que toda fuente sea rastreable
    reference: String,
    #[allow(dead_code)]
    consulted: String,
    #[allow(dead_code)]
    note: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAccount {
    #[allow(dead_code)]
    broker: String,
    funding_cutoff: RawSourced,
    funding_cutoff_timezone: String,
    margin_closeout_pct: RawSourced,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInstrument {
    name: String,
    asset_class: String,
    currency: String,
    exchange: String,
    exchange_timezone: String,
    max_leverage: String,
    epic: RawSourced,
    margin_retail_pct: RawSourced,
    spread_min_points: RawSourced,
    spread_typical_points: RawSourced,
    value_per_point: RawSourced,
    min_deal_size: RawSourced,
    tick: RawSourced,
    funding_admin_fee_pct: RawSourced,
    funding_day_count: RawSourced,
    funding_benchmark: RawSourced,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShareCosts {
    currency: String,
    commission_per_share: Option<RawSourced>,
    commission_pct: Option<RawSourced>,
    commission_min: RawSourced,
    margin_retail_pct: RawSourced,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCatalog {
    sources: BTreeMap<String, RawSource>,
    account: RawAccount,
    instruments: BTreeMap<String, RawInstrument>,
    share_costs: BTreeMap<String, RawShareCosts>,
}

/// Convierte y valida un campo con fuente.
struct Ctx<'a> {
    sources: &'a BTreeMap<String, RawSource>,
}

impl Ctx<'_> {
    fn err(item: &str, field: &str, problem: impl Into<String>) -> CatalogError {
        CatalogError::Invalid {
            item: item.into(),
            field: field.into(),
            problem: problem.into(),
        }
    }

    fn sourced<T>(
        &self,
        item: &str,
        field: &str,
        raw: RawSourced,
        parse: impl Fn(&str) -> Result<T, String>,
    ) -> Result<Sourced<T>, CatalogError> {
        match (raw.value, raw.source, raw.pending) {
            (Some(v), Some(src), None) => {
                if !self.sources.contains_key(&src) {
                    return Err(Self::err(
                        item,
                        field,
                        format!("la fuente {src:?} no está en [sources]"),
                    ));
                }
                let value = parse(&v).map_err(|e| Self::err(item, field, e))?;
                Ok(Sourced::Known { value, source: src })
            }
            (None, None, Some(reason)) if !reason.trim().is_empty() => {
                Ok(Sourced::Pending { reason })
            }
            _ => Err(Self::err(
                item,
                field,
                "usar { value, source } o { pending = \"motivo\" }",
            )),
        }
    }
}

fn positive(s: &str) -> Result<Decimal, String> {
    let d = Decimal::from_str(s).map_err(|e| format!("{s:?} no es un decimal: {e}"))?;
    if d > Decimal::ZERO {
        Ok(d)
    } else {
        Err(format!("{s} tiene que ser mayor que 0"))
    }
}

fn pct(s: &str) -> Result<Decimal, String> {
    positive(s).and_then(|d| {
        if d <= Decimal::ONE_HUNDRED {
            Ok(d)
        } else {
            Err(format!("{s} % supera 100"))
        }
    })
}

fn text(s: &str) -> Result<String, String> {
    if s.trim().is_empty() {
        Err("texto vacío".into())
    } else {
        Ok(s.to_owned())
    }
}

impl Catalog {
    pub fn from_toml(toml_text: &str) -> Result<Self, CatalogError> {
        let raw: RawCatalog =
            toml::from_str(toml_text).map_err(|e| CatalogError::Parse(e.to_string()))?;
        let cx = Ctx {
            sources: &raw.sources,
        };

        let acc = raw.account;
        cx.sourced("account", "funding_cutoff", acc.funding_cutoff, |s| {
            chrono::NaiveTime::parse_from_str(s, "%H:%M").map_err(|_| format!("{s:?} no es HH:MM"))
        })?;
        Tz::from_str(&acc.funding_cutoff_timezone).map_err(|_| {
            Ctx::err(
                "account",
                "funding_cutoff_timezone",
                "zona horaria desconocida",
            )
        })?;
        let margin_closeout_pct = cx.sourced(
            "account",
            "margin_closeout_pct",
            acc.margin_closeout_pct,
            pct,
        )?;

        let mut instruments = BTreeMap::new();
        for (key, r) in raw.instruments {
            let k = key.as_str();
            let bad = |f: &str, e: String| Ctx::err(k, f, e);
            let id: InstrumentId = key.parse().map_err(|e| bad("id", format!("{e}")))?;
            let currency: Currency = r
                .currency
                .parse()
                .map_err(|e| bad("currency", format!("{e}")))?;
            let asset_class: AssetClass =
                r.asset_class.parse().map_err(|e| bad("asset_class", e))?;
            Tz::from_str(&r.exchange_timezone)
                .map_err(|_| bad("exchange_timezone", "zona horaria desconocida".into()))?;
            let max_leverage = positive(&r.max_leverage).map_err(|e| bad("max_leverage", e))?;
            let terms = BrokerTerms {
                epic: cx.sourced(k, "epic", r.epic, text)?,
                margin_retail_pct: cx.sourced(k, "margin_retail_pct", r.margin_retail_pct, pct)?,
                spread_min_points: cx.sourced(
                    k,
                    "spread_min_points",
                    r.spread_min_points,
                    positive,
                )?,
                spread_typical_points: cx.sourced(
                    k,
                    "spread_typical_points",
                    r.spread_typical_points,
                    positive,
                )?,
                value_per_point: cx.sourced(k, "value_per_point", r.value_per_point, positive)?,
                min_deal_size: cx.sourced(k, "min_deal_size", r.min_deal_size, positive)?,
                tick: cx.sourced(k, "tick", r.tick, positive)?,
                funding_admin_fee_pct: cx.sourced(
                    k,
                    "funding_admin_fee_pct",
                    r.funding_admin_fee_pct,
                    pct,
                )?,
                funding_day_count: cx.sourced(
                    k,
                    "funding_day_count",
                    r.funding_day_count,
                    |s| {
                        positive(s).and_then(|d| {
                            if d == Decimal::from(360) || d == Decimal::from(365) {
                                Ok(d)
                            } else {
                                Err("tiene que ser 360 o 365".into())
                            }
                        })
                    },
                )?,
                funding_benchmark: cx.sourced(k, "funding_benchmark", r.funding_benchmark, text)?,
            };
            // El tope propio nunca puede superar el apalancamiento regulatorio (100 / margen).
            if let Some(m) = terms.margin_retail_pct.value()
                && max_leverage > Decimal::ONE_HUNDRED / m
            {
                return Err(bad(
                    "max_leverage",
                    format!(
                        "{max_leverage} supera el regulatorio {}",
                        Decimal::ONE_HUNDRED / m
                    ),
                ));
            }
            let instrument = Instrument {
                id: id.clone(),
                name: r.name,
                asset_class,
                currency,
                exchange_timezone: r.exchange_timezone,
            };
            instruments.insert(
                id,
                InstrumentSpec {
                    instrument,
                    exchange: r.exchange,
                    max_leverage,
                    terms,
                },
            );
        }

        for (key, s) in raw.share_costs {
            let k = format!("share_costs.{key}");
            s.currency
                .parse::<Currency>()
                .map_err(|e| Ctx::err(&k, "currency", format!("{e}")))?;
            match (s.commission_per_share, s.commission_pct) {
                (Some(c), None) => cx
                    .sourced(&k, "commission_per_share", c, positive)
                    .map(|_| ())?,
                (None, Some(c)) => cx.sourced(&k, "commission_pct", c, pct).map(|_| ())?,
                _ => {
                    return Err(Ctx::err(
                        &k,
                        "commission",
                        "usar commission_per_share o commission_pct, no los dos",
                    ));
                }
            }
            cx.sourced(&k, "commission_min", s.commission_min, positive)?;
            cx.sourced(&k, "margin_retail_pct", s.margin_retail_pct, pct)?;
        }

        Ok(Self {
            instruments,
            margin_closeout_pct,
        })
    }

    /// Instrumento → bolsa, para [`crate::ExchangeCalendar::load`].
    pub fn exchange_map(&self) -> HashMap<InstrumentId, String> {
        self.instruments
            .iter()
            .map(|(id, s)| (id.clone(), s.exchange.clone()))
            .collect()
    }

    /// Todos los campos pendientes de confirmar.
    pub fn pending(&self) -> Vec<PendingField> {
        let mut out = Vec::new();
        if let Sourced::Pending { reason } = &self.margin_closeout_pct {
            out.push(PendingField {
                item: "account".into(),
                field: "margin_closeout_pct",
                reason: reason.clone(),
            });
        }
        for spec in self.instruments.values() {
            out.extend(spec.pending());
        }
        out
    }

    /// `Ok` solo si están todos los costos (spec §11). El `epic` no cuenta: solo hace falta
    /// para conectarse en vivo, no para simular.
    pub fn ready_for_backtest(&self) -> Result<(), Vec<PendingField>> {
        let missing: Vec<_> = self
            .pending()
            .into_iter()
            .filter(|p| p.field != "epic")
            .collect();
        if missing.is_empty() {
            Ok(())
        } else {
            Err(missing)
        }
    }
}
