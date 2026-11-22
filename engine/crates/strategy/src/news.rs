//! Ajuste de noticias del puntaje y alerta de salida (spec §9, #130).
//!
//! Reglas decididas por el dueño en el #130 (22-11-2026):
//! 1. Por noticia y por instrumento: `redondear(2 × sentimiento × relevancia)`.
//! 2. Un ±0,5 exacto se redondea **hacia cero**: una señal tibia no suma.
//! 3. Las noticias de la ventana se **suman** y el total se recorta a
//!    `[score.news_min, score.news_max]`.
//! 4. En un corto el ajuste **cambia de signo**: el puntaje mide la confluencia a favor de la
//!    dirección propuesta.
//!
//! Sin look-ahead: en la vela que cierra en `at` solo cuentan las interpretaciones que ya
//! existían (`interpreted_at <= at`). La ventana de 4 h se mide desde la **publicación** de la
//! noticia. Si la misma noticia tiene varias interpretaciones disponibles (otra versión del
//! intérprete), cuenta solo la más reciente: una noticia no se suma dos veces.

use std::collections::BTreeMap;

use cfds_core::{InstrumentId, NewsInterpretation};
use cfds_liquidity::Side;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};
use serde::Deserialize;

use crate::config::{NewsConfig, ScoreConfig};

/// Fecha de corte del modelo intérprete (ADR 0008 §5, #194), de `[model].training_cutoff` en
/// `config/news_interpreter.toml`. Un modelo entrenado después de una noticia puede "saber" qué
/// pasó después: en el backtest solo cuentan las noticias publicadas **después** de esta fecha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterpreterCutoff {
    pub training_cutoff: NaiveDate,
}

/// El archivo lo comparte Python (dueño del intérprete): acá solo se lee `[model]` y se ignora
/// el resto, que valida `cfds_intel.news.ollama.load_config`.
#[derive(Deserialize)]
struct RawInterpreterFile {
    model: RawInterpreterModel,
}

#[derive(Deserialize)]
struct RawInterpreterModel {
    training_cutoff: Option<String>,
}

impl InterpreterCutoff {
    /// Lee y valida `config/news_interpreter.toml`. La clave es obligatoria: sin ella no se sabe
    /// desde cuándo las interpretaciones no tienen look-ahead.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        let raw: RawInterpreterFile = toml::from_str(text)
            .map_err(|e| format!("config/news_interpreter.toml no es válido: {e}"))?;
        let value = raw.model.training_cutoff.ok_or(
            "config/news_interpreter.toml, model.training_cutoff: falta (ADR 0008 §5: fecha de \
             corte de entrenamiento del modelo o, si no se publica, su fecha de publicación)",
        )?;
        let training_cutoff = NaiveDate::parse_from_str(&value, "%Y-%m-%d").map_err(|_| {
            format!(
                "config/news_interpreter.toml, model.training_cutoff: {value:?} no es AAAA-MM-DD"
            )
        })?;
        Ok(Self { training_cutoff })
    }

    /// Primer instante en que una noticia publicada ya cuenta en el backtest: las 00:00 UTC del
    /// día siguiente al corte (publicada **después** de la fecha, nunca el mismo día).
    pub fn backtest_published_from(&self) -> DateTime<Utc> {
        let next = self
            .training_cutoff
            .succ_opt()
            .unwrap_or(self.training_cutoff);
        next.and_time(chrono::NaiveTime::MIN).and_utc()
    }
}

/// Ajuste de una noticia para un instrumento, antes de sumar y recortar.
pub fn single_adjustment(sentiment: Decimal, relevance: Decimal) -> i32 {
    let raw = Decimal::TWO * sentiment * relevance;
    let rounded = raw.round_dp_with_strategy(0, RoundingStrategy::MidpointTowardZero);
    // |2 × s × r| ≤ 2 con s ∈ [−1, 1] y r ∈ [0, 1]: siempre entra en i32.
    rounded.to_i32().unwrap_or(0)
}

/// La interpretación disponible más reciente de cada noticia en `at`, si la noticia es de la
/// ventana (`at − window < published_at <= at`).
fn latest_per_news(
    items: &[NewsInterpretation],
    at: DateTime<Utc>,
    window: TimeDelta,
) -> impl Iterator<Item = &NewsInterpretation> {
    let mut latest: BTreeMap<(&str, &str), &NewsInterpretation> = BTreeMap::new();
    for n in items
        .iter()
        .filter(|n| n.interpreted_at <= at && n.published_at > at - window)
    {
        let key = (n.source.as_str(), n.item_id.as_str());
        if latest
            .get(&key)
            .is_none_or(|old| n.interpreted_at > old.interpreted_at)
        {
            latest.insert(key, n);
        }
    }
    latest.into_values()
}

/// Ajuste de noticias del puntaje para `instrument` y `side` en la vela que cierra en `at`.
pub fn news_adjustment(
    items: &[NewsInterpretation],
    instrument: &InstrumentId,
    side: Side,
    at: DateTime<Utc>,
    news: &NewsConfig,
    score: &ScoreConfig,
) -> i32 {
    let total: i32 = latest_per_news(items, at, news.window)
        .flat_map(|n| n.impacts.iter())
        .filter(|i| &i.instrument_id == instrument)
        .map(|i| single_adjustment(i.sentiment, i.relevance))
        .sum();
    let signed = match side {
        Side::Long => total,
        Side::Short => -total,
    };
    signed.clamp(score.news_min, score.news_max)
}

/// Noticias que disparan la alerta de salida de una posición abierta (spec §9): las que
/// llegaron en `(since, at]`, con relevancia y |sentimiento| sobre los umbrales y el
/// sentimiento **en contra** de la posición. No cierra nada: solo avisa al responsable.
pub fn exit_alerts<'a>(
    items: &'a [NewsInterpretation],
    instrument: &InstrumentId,
    position: Side,
    since: DateTime<Utc>,
    at: DateTime<Utc>,
    news: &NewsConfig,
) -> Vec<&'a NewsInterpretation> {
    items
        .iter()
        .filter(|n| n.interpreted_at > since && n.interpreted_at <= at)
        .filter(|n| {
            n.impacts.iter().any(|i| {
                let against = match position {
                    Side::Long => i.sentiment.is_sign_negative() && !i.sentiment.is_zero(),
                    Side::Short => i.sentiment.is_sign_positive() && !i.sentiment.is_zero(),
                };
                &i.instrument_id == instrument
                    && against
                    && i.relevance >= news.alert_min_relevance
                    && i.sentiment.abs() >= news.alert_min_abs_sentiment
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cfds_core::{Interpreter, NewsImpact};
    use rust_decimal::dec;

    fn t(s: &str) -> DateTime<Utc> {
        s.parse().unwrap()
    }

    fn id(s: &str) -> InstrumentId {
        s.parse().unwrap()
    }

    fn cfg() -> (NewsConfig, ScoreConfig) {
        let c = crate::StrategyConfig::from_toml(include_str!("../../../../config/strategy.toml"))
            .unwrap();
        (c.news, c.score)
    }

    fn news(
        item: &str,
        published: &str,
        interpreted: &str,
        impacts: &[(&str, Decimal, Decimal)],
    ) -> NewsInterpretation {
        NewsInterpretation {
            interpretation_id: "a".repeat(64),
            source: "ecb".into(),
            item_id: item.into(),
            published_at: t(published),
            interpreted_at: t(interpreted),
            interpreter: Interpreter {
                name: "local-llm".into(),
                version: "1".into(),
                model: None,
                prompt_sha256: None,
            },
            summary_es: "Resumen.".into(),
            impacts: impacts
                .iter()
                .map(|&(i, s, r)| NewsImpact {
                    instrument_id: id(i),
                    sentiment: s,
                    relevance: r,
                })
                .collect(),
        }
    }

    const AT: &str = "2026-11-20T16:00:00Z";

    #[test]
    fn one_news_rounds_two_times_sentiment_times_relevance() {
        assert_eq!(single_adjustment(dec!(1), dec!(1)), 2);
        assert_eq!(single_adjustment(dec!(-1), dec!(1)), -2);
        assert_eq!(single_adjustment(dec!(0.4), dec!(1)), 1); // 0,8 → 1
        assert_eq!(single_adjustment(dec!(0.2), dec!(1)), 0); // 0,4 → 0
        assert_eq!(single_adjustment(dec!(0), dec!(1)), 0);
    }

    #[test]
    fn exact_halves_round_toward_zero() {
        assert_eq!(single_adjustment(dec!(0.5), dec!(0.5)), 0); // 0,5 → 0
        assert_eq!(single_adjustment(dec!(-0.5), dec!(0.5)), 0); // −0,5 → 0
        assert_eq!(single_adjustment(dec!(0.75), dec!(1)), 1); // 1,5 → 1
        assert_eq!(single_adjustment(dec!(-0.75), dec!(1)), -1); // −1,5 → −1
        assert_eq!(single_adjustment(dec!(0.7501), dec!(1)), 2); // 1,5002 → 2
    }

    #[test]
    fn several_news_add_up_and_are_clipped() {
        let (n, s) = cfg();
        let items = [
            news(
                "a",
                "2026-11-20T13:00:00Z",
                "2026-11-20T13:01:00Z",
                &[("US100", dec!(-0.6), dec!(1))],
            ),
            news(
                "b",
                "2026-11-20T14:00:00Z",
                "2026-11-20T14:01:00Z",
                &[("US100", dec!(-0.6), dec!(1))],
            ),
            news(
                "c",
                "2026-11-20T15:00:00Z",
                "2026-11-20T15:01:00Z",
                &[("US100", dec!(-1), dec!(1))],
            ),
        ];
        // −1 −1 −2 = −4 → recortado a −2.
        assert_eq!(
            news_adjustment(&items, &id("US100"), Side::Long, t(AT), &n, &s),
            -2
        );
        assert_eq!(
            news_adjustment(&items[..1], &id("US100"), Side::Long, t(AT), &n, &s),
            -1
        );
    }

    #[test]
    fn a_short_flips_the_sign() {
        let (n, s) = cfg();
        let items = [news(
            "a",
            "2026-11-20T15:00:00Z",
            "2026-11-20T15:01:00Z",
            &[("US100", dec!(-0.6), dec!(1))],
        )];
        assert_eq!(
            news_adjustment(&items, &id("US100"), Side::Long, t(AT), &n, &s),
            -1
        );
        assert_eq!(
            news_adjustment(&items, &id("US100"), Side::Short, t(AT), &n, &s),
            1
        );
    }

    #[test]
    fn no_look_ahead_and_only_the_last_four_hours_by_publication() {
        let (n, s) = cfg();
        let x = &[("US100", dec!(1), dec!(1))];
        let items = [
            // Interpretada después del cierre de la vela: todavía no existe.
            news("late", "2026-11-20T15:30:00Z", "2026-11-20T16:00:01Z", x),
            // Publicada hace exactamente 4 h: fuera de la ventana.
            news("old", "2026-11-20T12:00:00Z", "2026-11-20T12:01:00Z", x),
            // Publicada hace 5 h e interpretada hace poco: cuenta la publicación, queda fuera.
            news("reread", "2026-11-20T11:00:00Z", "2026-11-20T15:59:00Z", x),
        ];
        assert_eq!(
            news_adjustment(&items, &id("US100"), Side::Long, t(AT), &n, &s),
            0
        );
        let inside = [news(
            "ok",
            "2026-11-20T12:00:01Z",
            "2026-11-20T16:00:00Z",
            x,
        )];
        assert_eq!(
            news_adjustment(&inside, &id("US100"), Side::Long, t(AT), &n, &s),
            2
        );
    }

    #[test]
    fn other_instruments_and_no_news_give_zero() {
        let (n, s) = cfg();
        let items = [news(
            "a",
            "2026-11-20T15:00:00Z",
            "2026-11-20T15:01:00Z",
            &[("DE40", dec!(1), dec!(1))],
        )];
        assert_eq!(
            news_adjustment(&items, &id("US100"), Side::Long, t(AT), &n, &s),
            0
        );
        assert_eq!(
            news_adjustment(&[], &id("US100"), Side::Long, t(AT), &n, &s),
            0
        );
    }

    #[test]
    fn a_news_reinterpreted_counts_once_with_its_latest_reading() {
        let (n, s) = cfg();
        let v1 = news(
            "a",
            "2026-11-20T15:00:00Z",
            "2026-11-20T15:01:00Z",
            &[("US100", dec!(-1), dec!(1))],
        );
        let mut v2 = news(
            "a",
            "2026-11-20T15:00:00Z",
            "2026-11-20T15:30:00Z",
            &[("US100", dec!(-0.4), dec!(1))],
        );
        v2.interpreter.version = "2".into();
        assert_eq!(
            news_adjustment(
                &[v1.clone(), v2.clone()],
                &id("US100"),
                Side::Long,
                t(AT),
                &n,
                &s
            ),
            -1
        );
        // Antes de las 15:30 solo existía la primera lectura.
        assert_eq!(
            news_adjustment(
                &[v1, v2],
                &id("US100"),
                Side::Long,
                t("2026-11-20T15:15:00Z"),
                &n,
                &s
            ),
            -2
        );
    }

    #[test]
    fn exit_alert_only_for_strong_relevant_news_against_the_position() {
        let (n, _) = cfg();
        let items = [
            news(
                "strong",
                "2026-11-20T15:00:00Z",
                "2026-11-20T15:10:00Z",
                &[("US100", dec!(-0.7), dec!(0.7))],
            ),
            news(
                "weak",
                "2026-11-20T15:00:00Z",
                "2026-11-20T15:10:00Z",
                &[("US100", dec!(-0.69), dec!(0.9))],
            ),
            news(
                "irrelevant",
                "2026-11-20T15:00:00Z",
                "2026-11-20T15:10:00Z",
                &[("US100", dec!(-0.9), dec!(0.69))],
            ),
            news(
                "other",
                "2026-11-20T15:00:00Z",
                "2026-11-20T15:10:00Z",
                &[("DE40", dec!(-1), dec!(1))],
            ),
            news(
                "before",
                "2026-11-20T14:00:00Z",
                "2026-11-20T14:59:00Z",
                &[("US100", dec!(-1), dec!(1))],
            ),
        ];
        let since = t("2026-11-20T15:00:00Z");
        let got: Vec<_> = exit_alerts(&items, &id("US100"), Side::Long, since, t(AT), &n)
            .iter()
            .map(|x| x.item_id.as_str())
            .collect();
        assert_eq!(got, ["strong"]);
        // Para un corto, la misma noticia negativa está a favor: no hay alerta.
        assert!(exit_alerts(&items, &id("US100"), Side::Short, since, t(AT), &n).is_empty());
    }

    #[test]
    fn the_interpreter_cutoff_is_read_and_required() {
        let real = include_str!("../../../../config/news_interpreter.toml");
        let c = InterpreterCutoff::from_toml(real).unwrap();
        assert_eq!(
            c.training_cutoff,
            NaiveDate::from_ymd_opt(2025, 12, 2).unwrap()
        );
        // "Después de la fecha": desde el día siguiente, a las 00:00 UTC.
        assert_eq!(
            c.backtest_published_from(),
            "2025-12-03T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
        );
        let line = "training_cutoff = \"2025-12-02\"";
        assert!(real.contains(line));
        let missing = InterpreterCutoff::from_toml(&real.replace(line, "")).unwrap_err();
        assert!(missing.contains("falta"), "{missing}");
        for bad in ["training_cutoff = \"2025-13-02\"", "training_cutoff = \"\""] {
            assert!(
                InterpreterCutoff::from_toml(&real.replace(line, bad)).is_err(),
                "{bad}"
            );
        }
        assert!(InterpreterCutoff::from_toml("[ollama]\nurl = \"x\"\n").is_err());
    }
}
