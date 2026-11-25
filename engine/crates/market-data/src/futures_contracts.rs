//! Especificaciones de los futuros estándar del programa 6 (#672), leídas de
//! `config/futures_contracts.toml`: multiplicador, tick, valor del tick, moneda, bolsa, horario,
//! meses listados y la comisión de IBKR, cada dato con su fuente en el archivo.
//!
//! Son datos de referencia verificados (IBKR `reqContractDetails` y la página de CME de cada
//! contrato), no parámetros del programa. Se validan al leer: un archivo inválido no se usa.

use std::collections::BTreeSet;

use rust_decimal::Decimal;
use serde::Deserialize;

/// Clase de activo del contrato (la del borrador del programa 6, §4.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Equity,
    Bond,
    Fx,
    Commodity,
}

/// Un futuro estándar.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuturesContract {
    /// Raíz del futuro (por ejemplo `ES`, `6E`).
    pub id: String,
    pub asset_class: AssetClass,
    /// Símbolo que usa IBKR (por ejemplo `EUR` para 6E).
    pub ibkr_symbol: String,
    pub trading_class: String,
    pub exchange: String,
    pub currency: String,
    pub multiplier: Decimal,
    pub tick_size: Decimal,
    /// Valor de un tick en `currency`: siempre `tick_size × multiplier`.
    pub tick_value: Decimal,
    /// Zona horaria del horario (la que informa IBKR).
    pub timezone: String,
    /// Apertura de la sesión (HH:MM, el día anterior) y cierre (HH:MM), en `timezone`.
    pub session_open: String,
    pub session_close: String,
    /// Meses de los vencimientos listados por IBKR, en letras de CME (F..Z), sin repetir.
    pub listed_months: String,
    pub next_contract: String,
    /// Último momento de negociación del vencimiento más cercano, en `timezone`.
    pub next_last_trade: String,
    /// Página oficial de la bolsa con la que se comparó.
    pub spec_url: String,
}

/// Un escalón del plan escalonado de IBKR: hasta `up_to_contracts` en el mes (sin tope, el último).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommissionTier {
    pub up_to_contracts: Option<u32>,
    pub usd: Decimal,
}

/// Comisión de IBKR por contrato y por lado para futuros estándar de EE. UU.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IbkrCommission {
    pub source_url: String,
    pub fixed_usd: Decimal,
    /// Si la comisión incluye las tasas de bolsa y regulatorias (en IBKR, no).
    pub exchange_fees_included: bool,
    pub tiered: Vec<CommissionTier>,
}

/// Todo el archivo.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FuturesContracts {
    /// Fecha en que se verificaron los datos (AAAA-MM-DD).
    pub checked_on: String,
    pub commission_ibkr_us: IbkrCommission,
    pub contracts: Vec<FuturesContract>,
}

const MONTH_CODES: &str = "FGHJKMNQUVXZ";

fn hh_mm(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && s[..2].parse::<u8>().is_ok_and(|h| h < 24)
        && s[3..].parse::<u8>().is_ok_and(|m| m < 60)
}

impl FuturesContracts {
    /// Lee y valida `config/futures_contracts.toml`.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let me: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        me.validate()?;
        Ok(me)
    }

    /// El contrato con esa raíz.
    pub fn get(&self, id: &str) -> Option<&FuturesContract> {
        self.contracts.iter().find(|c| c.id == id)
    }

    fn validate(&self) -> Result<(), String> {
        if chrono::NaiveDate::parse_from_str(&self.checked_on, "%Y-%m-%d").is_err() {
            return Err(format!("checked_on inválido: {}", self.checked_on));
        }
        let c = &self.commission_ibkr_us;
        if !c.source_url.starts_with("https://") || c.fixed_usd <= Decimal::ZERO {
            return Err("commission_ibkr_us: fuente https y comisión fija > 0".into());
        }
        if c.tiered.is_empty() {
            return Err("commission_ibkr_us.tiered vacío".into());
        }
        let mut prev = 0u32;
        for (i, t) in c.tiered.iter().enumerate() {
            let last = i + 1 == c.tiered.len();
            if t.usd <= Decimal::ZERO {
                return Err(format!("commission_ibkr_us.tiered[{i}]: usd > 0"));
            }
            match (t.up_to_contracts, last) {
                (Some(n), false) if n > prev => prev = n,
                (None, true) => {}
                _ => {
                    return Err(format!(
                        "commission_ibkr_us.tiered[{i}]: topes crecientes y solo el último sin tope"
                    ));
                }
            }
        }
        if self.contracts.is_empty() {
            return Err("no hay contratos".into());
        }
        let mut ids = BTreeSet::new();
        for k in &self.contracts {
            let bad = |what: &str| Err(format!("{}: {what}", k.id));
            if k.id.trim().is_empty() || !ids.insert(k.id.as_str()) {
                return bad("id vacío o repetido");
            }
            for (name, v) in [
                ("ibkr_symbol", &k.ibkr_symbol),
                ("trading_class", &k.trading_class),
                ("exchange", &k.exchange),
                ("timezone", &k.timezone),
                ("next_contract", &k.next_contract),
                ("next_last_trade", &k.next_last_trade),
            ] {
                if v.trim().is_empty() {
                    return bad(&format!("{name} vacío"));
                }
            }
            if k.currency.len() != 3 || !k.currency.bytes().all(|b| b.is_ascii_uppercase()) {
                return bad("currency debe ser un código ISO de 3 letras");
            }
            if k.multiplier <= Decimal::ZERO || k.tick_size <= Decimal::ZERO {
                return bad("multiplier y tick_size > 0");
            }
            if k.tick_size * k.multiplier != k.tick_value {
                return bad("tick_value distinto de tick_size × multiplier");
            }
            if !hh_mm(&k.session_open) || !hh_mm(&k.session_close) {
                return bad("session_open y session_close en HH:MM");
            }
            if k.timezone.parse::<chrono_tz::Tz>().is_err() {
                return bad("timezone desconocida");
            }
            let months: Vec<char> = k.listed_months.chars().collect();
            let mut seen = BTreeSet::new();
            if months.is_empty()
                || months
                    .iter()
                    .any(|m| !MONTH_CODES.contains(*m) || !seen.insert(*m))
            {
                return bad("listed_months: letras de CME (F..Z) sin repetir");
            }
            if chrono::NaiveDateTime::parse_from_str(&k.next_last_trade, "%Y-%m-%d %H:%M").is_err()
            {
                return bad("next_last_trade en AAAA-MM-DD HH:MM");
            }
            if !k.spec_url.starts_with("https://") {
                return bad("spec_url debe ser https");
            }
        }
        Ok(())
    }
}
