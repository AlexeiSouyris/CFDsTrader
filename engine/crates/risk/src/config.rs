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

/// Cómo baja el riesgo por operación entre los puntos de drawdown (#671).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawdownMode {
    /// Interpola en línea recta entre puntos (desde 0 % de caída con factor 1).
    Linear,
    /// Escalones: desde cada punto vale su factor, hasta el siguiente.
    Step,
}

/// Desde `drawdown_pct` de caída desde el máximo, el riesgo por operación se multiplica por
/// `scale` (#671).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawdownPoint {
    pub drawdown_pct: Decimal,
    pub scale: Decimal,
}

/// Reducción gradual del riesgo por caída (#671). Sin puntos = apagada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DrawdownScaling {
    pub mode: DrawdownMode,
    pub points: Vec<DrawdownPoint>,
}

impl DrawdownScaling {
    /// Factor del riesgo por operación con `drawdown_pct` de caída (≥ 0). 1 sin puntos.
    pub fn scale(&self, drawdown_pct: Decimal) -> Decimal {
        let mut prev = DrawdownPoint {
            drawdown_pct: Decimal::ZERO,
            scale: Decimal::ONE,
        };
        for p in &self.points {
            if drawdown_pct < p.drawdown_pct {
                return match self.mode {
                    DrawdownMode::Step => prev.scale,
                    DrawdownMode::Linear => {
                        let t = (drawdown_pct - prev.drawdown_pct)
                            / (p.drawdown_pct - prev.drawdown_pct);
                        prev.scale + (p.scale - prev.scale) * t
                    }
                };
            }
            prev = *p;
        }
        prev.scale
    }
}

/// Volatilidad objetivo de la cuenta (#671, estilo Carver): el riesgo por operación se escala
/// por `target / realizada`, acotado entre `min_scale` y `max_scale`. Objetivo 0 = apagada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolTarget {
    /// Volatilidad anual objetivo del capital, en %. 0 = apagada.
    pub target_annual_pct: Decimal,
    pub min_scale: Decimal,
    pub max_scale: Decimal,
}

impl VolTarget {
    pub fn enabled(&self) -> bool {
        !self.target_annual_pct.is_zero()
    }

    /// Factor para una volatilidad anual realizada (en %). Sin dato (calentamiento) o apagada,
    /// 1 acotado a los topes: nunca se agranda el riesgo por falta de datos si `max_scale` < 1.
    pub fn scale(&self, realized_annual_pct: Option<Decimal>) -> Decimal {
        if !self.enabled() {
            return Decimal::ONE;
        }
        let raw = match realized_annual_pct {
            Some(v) if v > Decimal::ZERO => self.target_annual_pct / v,
            _ => Decimal::ONE,
        };
        raw.max(self.min_scale).min(self.max_scale)
    }
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
    /// Reducción gradual del riesgo por caída (#671). Sin puntos = apagada.
    pub drawdown_scaling: DrawdownScaling,
    /// Volatilidad objetivo de la cuenta (#671). Objetivo 0 = apagada.
    pub vol_target: VolTarget,
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
        struct Point {
            drawdown_pct: String,
            scale: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Scaling {
            mode: String,
            #[serde(default)]
            points: Vec<Point>,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Vol {
            target_annual_pct: String,
            min_scale: String,
            max_scale: String,
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            limits: Limits,
            stop_distance: StopDistance,
            gap: Gap,
            correlation: Correlation,
            drawdown_scaling: Scaling,
            vol_target: Vol,
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
            drawdown_scaling: DrawdownScaling {
                mode: match raw.drawdown_scaling.mode.as_str() {
                    "linear" => DrawdownMode::Linear,
                    "step" => DrawdownMode::Step,
                    other => {
                        return Err(format!(
                            "drawdown_scaling.mode {other:?}: tiene que ser \"linear\" o \"step\""
                        ));
                    }
                },
                points: raw
                    .drawdown_scaling
                    .points
                    .iter()
                    .map(|p| {
                        Ok(DrawdownPoint {
                            drawdown_pct: percent(
                                &p.drawdown_pct,
                                "drawdown_scaling.drawdown_pct",
                            )?,
                            scale: dec(&p.scale, "drawdown_scaling.scale")?,
                        })
                    })
                    .collect::<Result<Vec<_>, String>>()?,
            },
            vol_target: VolTarget {
                target_annual_pct: non_negative(
                    &raw.vol_target.target_annual_pct,
                    "vol_target.target_annual_pct",
                )?,
                min_scale: dec(&raw.vol_target.min_scale, "vol_target.min_scale")?,
                max_scale: dec(&raw.vol_target.max_scale, "vol_target.max_scale")?,
            },
        };
        if cfg.max_open_positions == 0 || cfg.max_positions_per_instrument == 0 {
            return Err("los máximos de posiciones tienen que ser mayores que 0".into());
        }
        if cfg.min_stop_atr >= cfg.max_stop_atr {
            return Err("stop_distance.min_atr tiene que ser menor que max_atr".into());
        }
        // La caída gradual solo baja el riesgo, de a poco, y termina antes del kill switch.
        let mut prev = DrawdownPoint {
            drawdown_pct: Decimal::ZERO,
            scale: Decimal::ONE,
        };
        for p in &cfg.drawdown_scaling.points {
            if p.drawdown_pct <= prev.drawdown_pct || p.scale > prev.scale {
                return Err(
                    "drawdown_scaling.points: drawdown creciente y factor que no sube (≤ 1)".into(),
                );
            }
            if p.drawdown_pct >= cfg.max_drawdown_pct {
                return Err(
                    "drawdown_scaling.points: cada punto tiene que estar antes de max_drawdown_pct"
                        .into(),
                );
            }
            prev = *p;
        }
        let v = cfg.vol_target;
        if v.min_scale > v.max_scale {
            return Err("vol_target: min_scale tiene que ser ≤ max_scale".into());
        }
        if !v.enabled() && (v.min_scale != Decimal::ONE || v.max_scale != Decimal::ONE) {
            return Err("vol_target apagada (objetivo 0): min_scale y max_scale van en 1".into());
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
    /// Todo lo de #670 y #671 apagado: los tests de la mecánica prenden una cosa por vez.
    const NEUTRAL: &str = include_str!("../tests/neutral_risk.toml");

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
                // #670 y #671: valores aprobados por el dueño el 25-11-2026 (#689).
                gap_atr: dec!(0.25),
                groups: vec![CorrelationGroup {
                    name: "us_equity_index".into(),
                    max_risk_pct: dec!(1.5),
                    instruments: ["US100", "US500", "US30", "US2000"]
                        .iter()
                        .map(|i| i.parse().unwrap())
                        .collect(),
                }],
                drawdown_scaling: DrawdownScaling {
                    mode: DrawdownMode::Linear,
                    points: [
                        (dec!(5), dec!(1)),
                        (dec!(10), dec!(0.5)),
                        (dec!(12.5), dec!(0.25))
                    ]
                    .into_iter()
                    .map(|(drawdown_pct, scale)| DrawdownPoint {
                        drawdown_pct,
                        scale,
                    })
                    .collect(),
                },
                // Sigue apagada: falta la volatilidad realizada de la cuenta.
                vol_target: VolTarget {
                    target_annual_pct: dec!(0),
                    min_scale: dec!(1),
                    max_scale: dec!(1),
                },
            }
        );
    }

    #[test]
    fn the_neutral_config_is_the_repository_one_with_everything_off() {
        // La config neutral de los tests no se desvía de los límites del repositorio: solo apaga
        // lo de #670 y #671.
        let mut real = RiskConfig::from_toml(REAL).unwrap();
        real.gap_atr = dec!(0);
        real.groups.clear();
        real.drawdown_scaling.points.clear();
        assert_eq!(real, RiskConfig::from_toml(NEUTRAL).unwrap());
    }

    const GROUP: &str = r#"
[[correlation.groups]]
name = "us_equity_index"
max_risk_pct = "1.5"
instruments = ["US100", "US500", "US30", "US2000"]
"#;

    #[test]
    fn reads_correlation_groups() {
        let cfg = RiskConfig::from_toml(&format!("{NEUTRAL}{GROUP}")).unwrap();
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
            format!("{NEUTRAL}{GROUP}{GROUP}"),
            format!("{NEUTRAL}{}", GROUP.replace(r#""1.5""#, r#""0""#)),
            format!("{NEUTRAL}{}", GROUP.replace(r#""1.5""#, r#""100""#)),
            format!(
                "{NEUTRAL}{}",
                GROUP.replace(r#", "US500", "US30", "US2000""#, "")
            ),
            format!("{NEUTRAL}{}", GROUP.replace(r#""US500""#, r#""US100""#)),
            format!("{NEUTRAL}{}", GROUP.replace(r#""US500""#, r#""us 500""#)),
            format!("{NEUTRAL}{}", GROUP.replace("us_equity_index", " ")),
            NEUTRAL.replace(r#"gap_atr = "0""#, r#"gap_atr = "-0.1""#),
            NEUTRAL.replace(r#"gap_atr = "0""#, r#"gap_atr = "x""#),
        ] {
            assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }

    const DD: &str = r#"points = [
  { drawdown_pct = "5", scale = "1" },
  { drawdown_pct = "10", scale = "0.5" },
  { drawdown_pct = "12.5", scale = "0.25" },
]
"#;

    fn with_points(mode: &str) -> RiskConfig {
        let text = NEUTRAL.replace(
            "[drawdown_scaling]\nmode = \"linear\"\n",
            &format!("[drawdown_scaling]\nmode = \"{mode}\"\n{DD}"),
        );
        assert_ne!(text, NEUTRAL);
        RiskConfig::from_toml(&text).unwrap()
    }

    #[test]
    fn drawdown_scaling_linear_and_step() {
        let lin = with_points("linear").drawdown_scaling;
        let step = with_points("step").drawdown_scaling;
        for (dd, l, s) in [
            (dec!(0), dec!(1), dec!(1)),
            (dec!(4.99), dec!(1), dec!(1)),
            (dec!(5), dec!(1), dec!(1)),
            (dec!(7.5), dec!(0.75), dec!(1)),
            (dec!(10), dec!(0.5), dec!(0.5)),
            (dec!(11.25), dec!(0.375), dec!(0.5)),
            (dec!(12.5), dec!(0.25), dec!(0.25)),
            (dec!(14.9), dec!(0.25), dec!(0.25)),
        ] {
            assert_eq!((lin.scale(dd), step.scale(dd)), (l, s), "{dd}");
        }
        // Sin puntos, 1 siempre.
        let off = RiskConfig::from_toml(NEUTRAL).unwrap().drawdown_scaling;
        assert_eq!(off.scale(dec!(14)), dec!(1));
    }

    #[test]
    fn vol_target_scales_and_caps() {
        let v = VolTarget {
            target_annual_pct: dec!(10),
            min_scale: dec!(0.5),
            max_scale: dec!(1),
        };
        assert_eq!(v.scale(Some(dec!(10))), dec!(1));
        assert_eq!(v.scale(Some(dec!(16))), dec!(0.625));
        assert_eq!(v.scale(Some(dec!(40))), dec!(0.5));
        assert_eq!(v.scale(Some(dec!(5))), dec!(1));
        assert_eq!(v.scale(None), dec!(1));
        assert_eq!(v.scale(Some(dec!(0))), dec!(1));
        let wide = VolTarget {
            max_scale: dec!(1.5),
            ..v
        };
        assert_eq!(wide.scale(Some(dec!(5))), dec!(1.5));
        let tight = VolTarget {
            max_scale: dec!(0.8),
            ..v
        };
        assert_eq!(tight.scale(None), dec!(0.8));
        let off = RiskConfig::from_toml(NEUTRAL).unwrap().vol_target;
        assert_eq!(off.scale(Some(dec!(99))), dec!(1));
    }

    #[test]
    fn rejects_invalid_scaling() {
        let pts = |p: &str| {
            NEUTRAL.replace(
                "[drawdown_scaling]\nmode = \"linear\"\n",
                &format!("[drawdown_scaling]\nmode = \"linear\"\npoints = [{p}]\n"),
            )
        };
        for bad in [
            pts(r#"{ drawdown_pct = "5", scale = "1.2" }"#),
            pts(r#"{ drawdown_pct = "5", scale = "0.5" }, { drawdown_pct = "8", scale = "0.6" }"#),
            pts(r#"{ drawdown_pct = "8", scale = "0.5" }, { drawdown_pct = "5", scale = "0.4" }"#),
            pts(r#"{ drawdown_pct = "15", scale = "0.5" }"#),
            pts(r#"{ drawdown_pct = "5", scale = "0" }"#),
            NEUTRAL.replace(r#"mode = "linear""#, r#"mode = "cubic""#),
            NEUTRAL.replace(r#"min_scale = "1""#, r#"min_scale = "0.5""#),
            NEUTRAL.replace(r#"target_annual_pct = "0""#, r#"target_annual_pct = "-1""#),
            NEUTRAL
                .replace(r#"target_annual_pct = "0""#, r#"target_annual_pct = "10""#)
                .replace(r#"min_scale = "1""#, r#"min_scale = "1.2""#),
        ] {
            assert_ne!(bad, NEUTRAL);
            assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn group_instruments_exist_in_the_catalog() {
        // Las dos listas del ejemplo y del repositorio usan ids de config/instruments.toml.
        let catalog = include_str!("../../../../config/instruments.toml");
        let cfg = RiskConfig::from_toml(&format!("{NEUTRAL}{GROUP}")).unwrap();
        let real = RiskConfig::from_toml(REAL).unwrap();
        assert!(!real.groups.is_empty());
        for g in cfg.groups.iter().chain(&real.groups) {
            for i in &g.instruments {
                assert!(catalog.contains(&format!("[instruments.{i}]")), "{i}");
            }
        }
    }

    #[test]
    fn rejects_invalid_values() {
        for bad in [
            NEUTRAL.replace(r#"risk_per_trade_pct = "1""#, r#"risk_per_trade_pct = "0""#),
            NEUTRAL.replace(r#"max_drawdown_pct = "15""#, r#"max_drawdown_pct = "100""#),
            NEUTRAL.replace("max_open_positions = 5", "max_open_positions = 0"),
            NEUTRAL.replace(r#"min_atr = "0.5""#, r#"min_atr = "3""#),
            NEUTRAL.replace("max_atr", "maximo_atr"),
        ] {
            assert!(RiskConfig::from_toml(&bad).is_err(), "{bad}");
        }
    }
}
