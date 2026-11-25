//! `config/risk.toml`, validado al cargar.

use std::str::FromStr;

use cfds_core::InstrumentId;
use rust_decimal::Decimal;
use serde::Deserialize;

/// Instrumentos que se mueven casi igual: su riesgo abierto se suma contra un tope común (#670).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrelationGroup {
    pub name: String,
    /// Tope del riesgo abierto del grupo (las posiciones abiertas más la nueva), en % del capital.
    pub max_risk_pct: Decimal,
    pub instruments: Vec<InstrumentId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskConfig {
    pub risk_per_trade_pct: Decimal,
    pub max_daily_loss_pct: Decimal,
    pub max_drawdown_pct: Decimal,
    pub max_open_positions: usize,
    pub max_positions_per_instrument: usize,
    pub min_stop_atr: Decimal,
    pub max_stop_atr: Decimal,
    /// Colchón por gaps, en múltiplos del ATR, que se suma a la distancia del stop al calcular el
    /// tamaño (#670). 0 = apagado: el tamaño se calcula solo con la distancia al stop.
    pub gap_atr: Decimal,
    /// Grupos correlacionados (#670). Vacío = apagado.
    pub groups: Vec<CorrelationGroup>,
}

impl RiskConfig {
    pub fn from_toml(text: &str) -> Result<Self, String> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Limits {
            risk_per_trade_pct: String,
            max_daily_loss_pct: String,
            max_drawdown_pct: String,
            max_open_positions: usize,
            max_positions_per_instrument: usize,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct StopDistance {
            min_atr: String,
            max_atr: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Gap {
            gap_atr: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Group {
            name: String,
            max_risk_pct: String,
            instruments: Vec<String>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Correlation {
            #[serde(default)]
            groups: Vec<Group>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            limits: Limits,
            stop_distance: StopDistance,
            gap: Gap,
            correlation: Correlation,
        }
        let raw: Raw = toml::from_str(text).map_err(|e| e.to_string())?;
        let dec = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = Decimal::from_str(s).map_err(|e| format!("{name}: {e}"))?;
            if d <= Decimal::ZERO {
                return Err(format!("{name} tiene que ser mayor que 0"));
            }
            Ok(d)
        };
        let l = raw.limits;
        let percent = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = dec(s, name)?;
            if d >= Decimal::ONE_HUNDRED {
                return Err(format!("{name} tiene que ser menor que 100"));
            }
            Ok(d)
        };
        let non_negative = |s: &str, name: &str| -> Result<Decimal, String> {
            let d = Decimal::from_str(s).map_err(|e| format!("{name}: {e}"))?;
            if d < Decimal::ZERO {
                return Err(format!("{name} no puede ser negativo"));
            }
            Ok(d)
        };
        let cfg = Self {
            risk_per_trade_pct: percent(&l.risk_per_trade_pct, "risk_per_trade_pct")?,
            max_daily_loss_pct: percent(&l.max_daily_loss_pct, "max_daily_loss_pct")?,
            max_drawdown_pct: percent(&l.max_drawdown_pct, "max_drawdown_pct")?,
            max_open_positions: l.max_open_positions,
            max_positions_per_instrument: l.max_positions_per_instrument,
            min_stop_atr: dec(&raw.stop_distance.min_atr, "min_atr")?,
            max_stop_atr: dec(&raw.stop_distance.max_atr, "max_atr")?,
            gap_atr: non_negative(&raw.gap.gap_atr, "gap_atr")?,
            groups: raw
                .correlation
                .groups
                .iter()
                .map(|g| {
                    let name = format!("grupo {:?}", g.name);
                    let instruments = g
                        .instruments
                        .iter()
                        .map(|i| InstrumentId::from_str(i).map_err(|e| format!("{name}: {e}")))
                        .collect::<Result<Vec<_>, _>>()?;
                    Ok(CorrelationGroup {
                        max_risk_pct: percent(&g.max_risk_pct, &format!("{name}: max_risk_pct"))?,
                        name: g.name.clone(),
                        instruments,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        };
        if cfg.max_open_positions == 0 || cfg.max_positions_per_instrument == 0 {
            return Err("los máximos de posiciones tienen que ser mayores que 0".into());
        }
        if cfg.min_stop_atr >= cfg.max_stop_atr {
            return Err("stop_distance.min_atr tiene que ser menor que max_atr".into());
        }
        let mut names = std::collections::HashSet::new();
        for g in &cfg.groups {
            if g.name.trim().is_empty() || !names.insert(g.name.as_str()) {
                return Err(format!("grupo {:?}: nombre vacío o repetido", g.name));
            }
            // Un grupo de uno no tiene con quién correlacionar: es un error de config.
            let unique: std::collections::HashSet<_> = g.instruments.iter().collect();
            if unique.len() < 2 || unique.len() != g.instruments.len() {
                return Err(format!(
                    "grupo {:?}: necesita al menos 2 instrumentos, sin repetir",
                    g.name
                ));
            }
        }
        Ok(cfg)
    }

    /// Los grupos que incluyen `instrument` (puede estar en más de uno, o en ninguno).
    pub fn groups_of<'a>(
        &'a self,
        instrument: &'a InstrumentId,
    ) -> impl Iterator<Item = &'a CorrelationGroup> + 'a {
        self.groups
            .iter()
            .filter(move |g| g.instruments.contains(instrument))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    const REAL: &str = include_str!("../../../../config/risk.toml");

    #[test]
    fn the_repository_config_matches_the_spec() {
        assert_eq!(
            RiskConfig::from_toml(REAL).unwrap(),
            RiskConfig {
                risk_per_trade_pct: dec!(1),
                max_daily_loss_pct: dec!(3),
                max_drawdown_pct: dec!(15),
                max_open_positions: 5,
                max_positions_per_instrument: 1,
                min_stop_atr: dec!(0.5),
                max_stop_atr: dec!(3),
                // #670: apagados hasta que el dueño apruebe valores.
                gap_atr: dec!(0),
                groups: vec![],
            }
        );
    }

    const GROUP: &str = r#"
[[correlation.groups]]
name = "us_equity_index"
max_risk_pct = "1.5"
instruments = ["US100", "US500", "US30", "US2000"]
"#;

    #[test]
    fn reads_correlation_groups() {
        let cfg = RiskConfig::from_toml(&format!("{REAL}{GROUP}")).unwrap();
        assert_eq!(cfg.groups.len(), 1);
        let g = &cfg.groups[0];
        assert_eq!(
            (g.name.as_str(), g.max_risk_pct),
            ("us_equity_index", dec!(1.5))
        );
        let us30: InstrumentId = "US30".parse().unwrap();
        let de40: InstrumentId = "DE40".parse().unwrap();
        assert_eq!(cfg.groups_of(&us30).count(), 1);
        assert_eq!(cfg.groups_of(&de40).count(), 0);
    }

    #[test]
    fn rejects_invalid_groups_and_gaps() {
        for bad in [
            format!("{REAL}{GROUP}{GROUP}"),
            format!("{REAL}{}", GROUP.replace(r#""1.5""#, r#""0""#)),
            format!("{REAL}{}", GROUP.replace(r#""1.5""#, r#""100""#)),
            format!(
                "{REAL}{}",
                GROUP.replace(r#", "US500", "US30", "US2000""#, "")
            ),
            format!("{REAL}{}", GROUP.replace(r#""US500""#, r#""US100""#)),
            format!("{REAL}{}", GROUP.replace(r#""US500""#, r#""us 500""#)),
            format!("{REAL}{}", GROUP.replace("us_equity_index", " ")),
            REAL.replace(r#"gap_atr = "0""#, r#"gap_atr = "-0.1""#),
            REAL.replace(r#"gap_atr = "0""#, r#"gap_atr = "x""#),
        ] {
            assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn group_instruments_exist_in_the_catalog() {
        // Las dos listas del ejemplo y del repositorio usan ids de config/instruments.toml.
        let catalog = include_str!("../../../../config/instruments.toml");
        let cfg = RiskConfig::from_toml(&format!("{REAL}{GROUP}")).unwrap();
        let real = RiskConfig::from_toml(REAL).unwrap();
        for g in cfg.groups.iter().chain(&real.groups) {
            for i in &g.instruments {
                assert!(catalog.contains(&format!("[instruments.{i}]")), "{i}");
            }
        }
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            REAL.replace(r#"risk_per_trade_pct = "1""#, r#"risk_per_trade_pct = "0""#),
            REAL.replace(r#"max_drawdown_pct = "15""#, r#"max_drawdown_pct = "100""#),
            REAL.replace("max_open_positions = 5", "max_open_positions = 0"),
            REAL.replace(r#"min_atr = "0.5""#, r#"min_atr = "3""#),
            REAL.replace("max_atr", "maximo_atr"),
        ] {
            assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}
