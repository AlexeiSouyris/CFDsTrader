//! `validate_spec`: una spec contra el catálogo y la separación de datos, con **todos** los
//! errores (`spec_validation.v1`) o la spec tipada y normalizada con sus hashes.

use std::fmt::Display;

use cfds_core::Timeframe;
use cfds_market_data::{DataSplit, Period};
use chrono::NaiveTime;
use rust_decimal::Decimal;

use super::catalog::{ParamKind, ParamSchema, Slot, StrategyCatalog, parse_time};
use super::spec::{Block, Mode, ParamValue, StrategySpec, family_sha256, spec_sha256};
use super::validation::{
    Average, EntryCondition, EntryFilter, ErrorCode, ExitRule, SpecError, SpecValidation, StopRule,
    ValidSpec,
};
use crate::research_period;

struct Ctx<'a> {
    catalog: &'a StrategyCatalog,
    timeframe: Timeframe,
    errors: Vec<SpecError>,
}

impl Ctx<'_> {
    fn err(&mut self, code: ErrorCode, path: &str, message: impl Into<String>) {
        self.errors.push(SpecError::new(code, path, message));
    }

    /// Valida un bloque en `slot` contra el catálogo: `true` si no tuvo errores. Normaliza los
    /// decimales de `block.params` (sin ceros de más) para el hash.
    fn block(&mut self, block: &mut Block, slot: Slot, path: &str) -> bool {
        let (kind, at_type, before) = (
            block.kind.clone(),
            format!("{path}/type"),
            self.errors.len(),
        );
        let Some(schema) = self.catalog.block(&kind) else {
            self.err(
                ErrorCode::UnknownBlock,
                &at_type,
                format!("bloque desconocido: {kind}"),
            );
            return false;
        };
        if schema.slot != slot {
            self.err(
                ErrorCode::WrongSlot,
                &at_type,
                format!("{kind} no va en este lugar"),
            );
            return false;
        }
        if !schema.timeframes.contains(&self.timeframe) {
            let tf = self.timeframe.as_str();
            self.err(
                ErrorCode::BlockTimeframe,
                &at_type,
                format!("{kind} no va con {tf}"),
            );
        }
        for name in block.params.keys() {
            if !schema.params.iter().any(|p| &p.name == name) {
                let at = format!("{path}/params/{name}");
                self.err(
                    ErrorCode::UnknownParam,
                    &at,
                    format!("{kind} no tiene {name}"),
                );
            }
        }
        for p in &schema.params {
            let at = format!("{path}/params/{}", p.name);
            match block.params.get_mut(&p.name).map(|v| check(p, v)) {
                None if p.required => self.err(ErrorCode::MissingParam, &at, "falta"),
                Some(Err((code, msg))) => self.err(code, &at, msg),
                _ => {}
            }
        }
        self.errors.len() == before
    }
}

/// Decimal del contrato (`^\d+(\.\d+)?$`).
fn contract_decimal(s: &str) -> Option<Decimal> {
    let (int, frac) = s.split_once('.').unwrap_or((s, "0"));
    let digits = |p: &str| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit());
    (digits(int) && digits(frac)).then(|| s.parse().ok())?
}

fn check(p: &ParamSchema, value: &mut ParamValue) -> Result<(), (ErrorCode, String)> {
    let wrong = |what: &str| (ErrorCode::ParamType, format!("tiene que ser {what}"));
    let out = |lo: &dyn Display, hi: &dyn Display| {
        (
            ErrorCode::ParamOutOfRange,
            format!("tiene que ir de {lo} a {hi}"),
        )
    };
    match (&p.kind, &*value) {
        (ParamKind::Integer { min, max, .. }, ParamValue::Integer(v)) => (min..=max)
            .contains(&v)
            .then_some(())
            .ok_or_else(|| out(min, max)),
        (ParamKind::Decimal { min, max, .. }, ParamValue::Text(s)) => {
            let d = contract_decimal(s).ok_or_else(|| wrong("un decimal como texto"))?;
            if !(min..=max).contains(&&d) {
                return Err(out(min, max));
            }
            *value = ParamValue::Text(d.normalize().to_string());
            Ok(())
        }
        (ParamKind::Time { .. }, ParamValue::Text(s)) => parse_time(s)
            .map(drop)
            .ok_or_else(|| wrong("una hora HH:MM")),
        (ParamKind::Enum { options, .. }, ParamValue::Text(s)) => match options.contains(s) {
            true => Ok(()),
            false => Err((ErrorCode::ParamOutOfRange, format!("opciones: {options:?}"))),
        },
        (ParamKind::Boolean { .. }, ParamValue::Boolean(_)) => Ok(()),
        (ParamKind::Integer { .. }, _) => Err(wrong("un entero")),
        (ParamKind::Decimal { .. }, _) => Err(wrong("un decimal como texto")),
        (ParamKind::Time { .. }, _) => Err(wrong("una hora HH:MM")),
        (ParamKind::Enum { .. }, _) => Err(wrong("una opción")),
        (ParamKind::Boolean { .. }, _) => Err(wrong("true o false")),
    }
}

/// Lectores de parámetros de un bloque ya validado.
fn int(b: &Block, k: &str) -> Option<usize> {
    match b.params.get(k) {
        Some(ParamValue::Integer(v)) => usize::try_from(*v).ok(),
        _ => None,
    }
}

fn text<'a>(b: &'a Block, k: &str) -> Option<&'a str> {
    match b.params.get(k) {
        Some(ParamValue::Text(v)) => Some(v),
        _ => None,
    }
}

fn dec(b: &Block, k: &str) -> Option<Decimal> {
    text(b, k)?.parse().ok()
}

fn time(b: &Block, k: &str) -> Option<NaiveTime> {
    parse_time(text(b, k)?)
}

fn entry(ctx: &mut Ctx, p: &Block, path: &str) -> Option<EntryCondition> {
    if p.kind == "opening_range_breakout" {
        return Some(EntryCondition::OpeningRangeBreakout {
            range_candles: u32::try_from(int(p, "range_candles")?).ok()?,
            last_breakout_close: time(p, "last_breakout_close")?,
        });
    }
    let average = match text(p, "average")? {
        "sma" => Average::Sma,
        _ => Average::Ema,
    };
    let (fast, slow) = (int(p, "fast")?, int(p, "slow")?);
    if fast >= slow {
        let at = format!("{path}/params/fast");
        ctx.err(
            ErrorCode::InvalidCombination,
            &at,
            "la rápida tiene que ser más corta",
        );
        return None;
    }
    Some(EntryCondition::MaCross {
        average,
        fast,
        slow,
    })
}

fn exit(ctx: &mut Ctx, p: &Block) -> Option<ExitRule> {
    let session_close = matches!(
        p.params.get("session_close"),
        Some(ParamValue::Boolean(true))
    );
    let max_candles = int(p, "max_candles");
    if max_candles.is_none() && !session_close {
        let msg = "hace falta max_candles o session_close";
        ctx.err(ErrorCode::InvalidCombination, "/exit/params", msg);
        return None;
    }
    let target_r = dec(p, "target_r");
    Some(ExitRule {
        max_candles,
        session_close,
        target_r,
    })
}

fn is_sha256(h: &str) -> bool {
    h.len() == 64
        && h.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Valida `spec` (ver el módulo). `split` es la separación de datos vigente: un período que
/// llega al fuera de muestra se rechaza con el guardia `research_period`.
pub fn validate_spec(
    spec: &StrategySpec,
    catalog: &StrategyCatalog,
    split: &DataSplit,
) -> Result<ValidSpec, Vec<SpecError>> {
    use ErrorCode as E;
    let mut s = spec.clone();
    let timeframe = s.market.timeframe;
    let mut ctx = Ctx {
        catalog,
        timeframe,
        errors: Vec::new(),
    };
    let l = &catalog.limits;
    let market = catalog
        .markets
        .iter()
        .find(|m| m.instrument_id == s.market.instrument_id && m.source == s.market.source);
    let period = Period {
        start: s.period.from,
        end: s.period.to,
    };
    let oos = split.out_of_sample.start;
    let prereg = s.preregistration_sha256.as_deref();
    let checks = [
        (
            s.catalog_version != catalog.catalog_version,
            E::CatalogVersion,
            "/catalog_version",
            format!("el motor usa el catálogo {}", catalog.catalog_version),
        ),
        (
            market.is_none(),
            E::UnknownMarket,
            "/market/instrument_id",
            "sin velas de esa fuente para ese instrumento".into(),
        ),
        (
            market.is_some_and(|m| !m.timeframes.contains(&timeframe)),
            E::TimeframeUnavailable,
            "/market/timeframe",
            "timeframe no disponible".into(),
        ),
        (
            s.entry.is_empty(),
            E::Malformed,
            "/entry",
            "hace falta una condición de entrada".into(),
        ),
        (
            s.entry.len() > l.max_entry_conditions,
            E::TooManyConditions,
            "/entry",
            format!("como mucho {} condiciones", l.max_entry_conditions),
        ),
        (
            s.filters.len() > l.max_filters,
            E::TooManyFilters,
            "/filters",
            format!("como mucho {} filtros", l.max_filters),
        ),
        (
            s.risk_pct <= Decimal::ZERO || s.risk_pct > l.max_risk_pct,
            E::RiskOutOfRange,
            "/risk_pct",
            format!(
                "mayor que 0 y como mucho {} % (config/risk.toml)",
                l.max_risk_pct
            ),
        ),
        (
            !(l.min_cost_multiplier..=l.max_cost_multiplier).contains(&s.cost_multiplier),
            E::CostOutOfRange,
            "/cost_multiplier",
            format!(
                "solo se pueden subir: de {} a {}",
                l.min_cost_multiplier, l.max_cost_multiplier
            ),
        ),
        (
            s.period.from >= s.period.to,
            E::PeriodInvalid,
            "/period",
            "from ≥ to".into(),
        ),
        (
            s.period.from < s.period.to && research_period(period, split).is_err(),
            E::PeriodTouchesHoldout,
            "/period/to",
            format!("llega al fuera de muestra ({oos})"),
        ),
        (
            !catalog.modes.contains(&s.mode),
            E::ModeUnavailable,
            "/mode",
            "el confirmatorio llega con el pre-registro (L13)".into(),
        ),
        (
            prereg.is_some_and(|h| !is_sha256(h)),
            E::Malformed,
            "/preregistration_sha256",
            "tiene que ser un SHA-256 en hexadecimal".into(),
        ),
        (
            prereg.is_some_and(is_sha256) && s.mode == Mode::Exploratory,
            E::InvalidCombination,
            "/preregistration_sha256",
            "una exploratoria no lleva pre-registro".into(),
        ),
    ];
    for (bad, code, path, msg) in checks {
        if bad {
            ctx.err(code, path, msg);
        }
    }
    let mut entries = Vec::new();
    for (k, b) in s.entry.iter_mut().enumerate() {
        let path = format!("/entry/{k}");
        if ctx.block(b, Slot::Entry, &path) {
            entries.extend(entry(&mut ctx, b, &path));
        }
    }
    let mut filters = Vec::new();
    for (k, b) in s.filters.iter_mut().enumerate() {
        if ctx.block(b, Slot::Filter, &format!("/filters/{k}")) {
            let hours = time(b, "from").zip(time(b, "to"));
            filters.extend(hours.map(|(from, to)| EntryFilter::EntryHoursUtc { from, to }));
        }
    }
    let stop = ctx.block(&mut s.stop, Slot::Stop, "/stop").then(|| {
        let (period, k) = (int(&s.stop, "period")?, dec(&s.stop, "k")?);
        Some(StopRule::Atr { period, k })
    });
    let exit = match ctx.block(&mut s.exit, Slot::Exit, "/exit") {
        true => exit(&mut ctx, &s.exit),
        false => None,
    };
    let (Some(Some(stop)), Some(exit), true) = (stop, exit, ctx.errors.is_empty()) else {
        return Err(ctx.errors);
    };
    s.risk_pct = s.risk_pct.normalize();
    s.cost_multiplier = s.cost_multiplier.normalize();
    let malformed = |e: serde_json::Error| vec![SpecError::new(E::Malformed, "", e.to_string())];
    Ok(ValidSpec {
        spec_sha256: spec_sha256(&s).map_err(malformed)?,
        family_sha256: family_sha256(&s),
        timeframe,
        spec: s,
        entry: entries,
        filters,
        stop,
        exit,
    })
}

/// Valida el texto de una spec y arma el `spec_validation.v1`: si el JSON no tiene la forma del
/// contrato, un solo error `malformed` con el mensaje del lector.
pub fn validate_spec_json(
    text: &str,
    catalog: &StrategyCatalog,
    split: &DataSplit,
) -> (SpecValidation, Option<ValidSpec>) {
    let parsed = serde_json::from_str::<StrategySpec>(text)
        .map_err(|e| vec![SpecError::new(ErrorCode::Malformed, "", e.to_string())]);
    let result = parsed.and_then(|spec| validate_spec(&spec, catalog, split));
    let catalog_version = catalog.catalog_version;
    let (spec_sha256, family_sha256) = match &result {
        Ok(v) => (Some(v.spec_sha256.clone()), Some(v.family_sha256.clone())),
        Err(_) => (None, None),
    };
    let (valid, errors) = match &result {
        Ok(_) => (true, Vec::new()),
        Err(e) => (false, e.clone()),
    };
    let validation = SpecValidation {
        valid,
        catalog_version,
        spec_sha256,
        family_sha256,
        errors,
    };
    (validation, result.ok())
}
