//! Régimen del VIX (#491): el VIX diario de Cboe (`vix_daily`, lo ingiere Python) clasificado con
//! umbrales de `config/analysis.toml` (`[vix]`). Es **contexto**: no entra al puntaje.
//!
//! ```text
//! calma:     VIX < normal_from
//! normal:    normal_from ≤ VIX < elevated_from
//! elevado:   elevated_from ≤ VIX < stress_from
//! estrés:    VIX ≥ stress_from
//! ```
//! Los umbrales iniciales (15, 20 y 30) son la convención habitual del mercado, no un ajuste.
//! Una vela ve el último valor **publicado** antes de su cierre (`published_at ≤ fin de la vela`,
//! ADR 0012): nunca el del día en curso. Una corrección de Cboe es otro valor con otra
//! `published_at`, así que vale recién desde que se publicó (sin repintado).

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VixRegime {
    Calm,
    Normal,
    Elevated,
    Stress,
}

impl VixRegime {
    pub fn as_str(self) -> &'static str {
        match self {
            VixRegime::Calm => "calm",
            VixRegime::Normal => "normal",
            VixRegime::Elevated => "elevated",
            VixRegime::Stress => "stress",
        }
    }
}

/// Umbrales, en puntos del VIX: `0 < normal_from < elevated_from < stress_from`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VixConfig {
    pub normal_from: Decimal,
    pub elevated_from: Decimal,
    pub stress_from: Decimal,
}

impl VixConfig {
    pub fn new(
        normal_from: Decimal,
        elevated_from: Decimal,
        stress_from: Decimal,
    ) -> Result<Self, String> {
        if !(Decimal::ZERO < normal_from
            && normal_from < elevated_from
            && elevated_from < stress_from)
        {
            return Err("vix: 0 < normal_from < elevated_from < stress_from".into());
        }
        Ok(Self {
            normal_from,
            elevated_from,
            stress_from,
        })
    }
}

pub fn vix_regime(close: Decimal, cfg: &VixConfig) -> VixRegime {
    if close >= cfg.stress_from {
        VixRegime::Stress
    } else if close >= cfg.elevated_from {
        VixRegime::Elevated
    } else if close >= cfg.normal_from {
        VixRegime::Normal
    } else {
        VixRegime::Calm
    }
}

/// Último cierre del VIX publicado en o antes de cada instante de `at` (en orden).
/// `published` = `(published_at, cierre)`, en orden de `published_at`.
pub fn vix_known_at(
    published: &[(DateTime<Utc>, Decimal)],
    at: &[DateTime<Utc>],
) -> Vec<Option<Decimal>> {
    let mut k = 0;
    at.iter()
        .map(|t| {
            while k < published.len() && published[k].0 <= *t {
                k += 1;
            }
            k.checked_sub(1).map(|i| published[i].1)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::dec;

    fn cfg() -> VixConfig {
        VixConfig::new(dec!(15), dec!(20), dec!(30)).unwrap()
    }

    #[test]
    fn thresholds_are_inclusive_from_below() {
        let r = |x| vix_regime(x, &cfg());
        assert_eq!(r(dec!(14.99)), VixRegime::Calm);
        assert_eq!(r(dec!(15)), VixRegime::Normal);
        assert_eq!(r(dec!(20)), VixRegime::Elevated);
        assert_eq!(r(dec!(29.99)), VixRegime::Elevated);
        assert_eq!(r(dec!(30)), VixRegime::Stress);
    }

    #[test]
    fn rejects_unordered_thresholds() {
        assert!(VixConfig::new(dec!(20), dec!(15), dec!(30)).is_err());
        assert!(VixConfig::new(dec!(0), dec!(15), dec!(30)).is_err());
        assert!(VixConfig::new(dec!(15), dec!(30), dec!(30)).is_err());
    }

    #[test]
    fn a_value_counts_only_from_its_publication() {
        let t = |s: &str| s.parse::<DateTime<Utc>>().unwrap();
        let published = [
            (t("2026-11-02T14:30:00Z"), dec!(18)),
            (t("2026-11-03T14:30:00Z"), dec!(22)),
            // Corrección del mismo día: vale recién desde que se publicó.
            (t("2026-11-03T20:00:00Z"), dec!(21.5)),
        ];
        let at = [
            t("2026-11-02T14:00:00Z"),
            t("2026-11-02T14:30:00Z"),
            t("2026-11-03T14:29:59Z"),
            t("2026-11-03T15:00:00Z"),
            t("2026-11-04T09:00:00Z"),
        ];
        assert_eq!(
            vix_known_at(&published, &at),
            vec![
                None,
                Some(dec!(18)),
                Some(dec!(18)),
                Some(dec!(22)),
                Some(dec!(21.5))
            ]
        );
        assert!(vix_known_at(&[], &at).iter().all(Option::is_none));
    }
}
