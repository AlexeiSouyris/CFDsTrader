//! Qué futuro de Databento corresponde a cada instrumento (#376), leído de
//! `config/databento.toml` (`[[symbols]]`: `dataset`, `symbol`, `instrument_id`).
//!
//! El motor lee `futures_bars_1m` filtrando por fuente, dataset, símbolo **e** instrumento: así
//! una fila ajena (de otro dataset o símbolo con el mismo `instrument_id`) nunca se mezcla.
//! El resto del archivo (descarga, presupuesto, publisher) lo valida y lo usa Python
//! (`cfds_intel.databento.config`); acá se ignora.

use std::collections::BTreeMap;

use cfds_core::InstrumentId;
use serde::Deserialize;

/// El futuro de un instrumento: dataset y símbolo de Databento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuturesFeed {
    /// Dataset de Databento (por ejemplo `GLBX.MDP3`).
    pub dataset: String,
    /// Símbolo continuo (por ejemplo `ES.c.0`).
    pub symbol: String,
}

/// Futuro por instrumento (CFD).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FuturesFeeds {
    pub by_instrument: BTreeMap<InstrumentId, FuturesFeed>,
}

#[derive(Deserialize)]
struct Raw {
    #[serde(default)]
    symbols: Vec<RawSymbol>,
}

#[derive(Deserialize)]
struct RawSymbol {
    dataset: String,
    symbol: String,
    instrument_id: InstrumentId,
}

impl FuturesFeeds {
    /// Lee `config/databento.toml`. Error si falta un campo, si un dataset o símbolo está vacío
    /// o si un instrumento aparece dos veces (no se sabría qué futuro leer).
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut by_instrument = BTreeMap::new();
        for s in raw.symbols {
            if s.dataset.trim().is_empty() || s.symbol.trim().is_empty() {
                return Err(format!(
                    "{}: dataset y symbol no pueden estar vacíos",
                    s.instrument_id
                ));
            }
            let feed = FuturesFeed {
                dataset: s.dataset,
                symbol: s.symbol,
            };
            if by_instrument
                .insert(s.instrument_id.clone(), feed)
                .is_some()
            {
                return Err(format!(
                    "{} aparece dos veces en [[symbols]]",
                    s.instrument_id
                ));
            }
        }
        Ok(Self { by_instrument })
    }

    /// El futuro de `instrument`, si tiene.
    pub fn get(&self, instrument: &InstrumentId) -> Option<&FuturesFeed> {
        self.by_instrument.get(instrument)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = include_str!("../../../../config/databento.toml");

    fn id(s: &str) -> InstrumentId {
        s.parse().unwrap()
    }

    #[test]
    fn the_real_config_maps_each_instrument_to_its_future() {
        let f = FuturesFeeds::from_toml(REAL).unwrap();
        assert_eq!(
            f.get(&id("US500")),
            Some(&FuturesFeed {
                dataset: "GLBX.MDP3".into(),
                symbol: "ES.c.0".into()
            })
        );
        assert_eq!(f.get(&id("DE40")).unwrap().dataset, "XEUR.EOBI");
        assert_eq!(f.get(&id("DE40")).unwrap().symbol, "FDAX.c.0");
        assert_eq!(f.by_instrument.len(), 4);
        assert_eq!(f.get(&id("UK100")), None);
    }

    #[test]
    fn rejects_duplicates_and_empty_fields() {
        let dup = r#"
            [[symbols]]
            dataset = "GLBX.MDP3"
            symbol = "ES.c.0"
            instrument_id = "US500"
            [[symbols]]
            dataset = "GLBX.MDP3"
            symbol = "MES.c.0"
            instrument_id = "US500"
        "#;
        assert!(
            FuturesFeeds::from_toml(dup)
                .unwrap_err()
                .contains("dos veces")
        );
        let empty = r#"
            [[symbols]]
            dataset = " "
            symbol = "ES.c.0"
            instrument_id = "US500"
        "#;
        assert!(
            FuturesFeeds::from_toml(empty)
                .unwrap_err()
                .contains("vacíos")
        );
        let missing = r#"
            [[symbols]]
            symbol = "ES.c.0"
            instrument_id = "US500"
        "#;
        assert!(FuturesFeeds::from_toml(missing).is_err());
    }
}
