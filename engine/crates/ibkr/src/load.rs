//! Carga incremental de velas cerradas de IBKR (#536): qué ventanas pedir, cuántas por corrida y
//! cada cuánto, sin pasar los límites de IBKR (ver `docs/analysis/2026-11-ibkr-medicion-historia.md`).
//! Funciones puras: la conexión y el guardado los hace el ejemplo `ibkr_load`.

use chrono::{DateTime, DurationRound, TimeDelta, Utc};
use rust_decimal::Decimal;

use crate::config::ContractKind;
use crate::session::{ApiError, BarSize, RawBar, SessionError};

/// `[load]` de `config/ibkr.toml`, ya validado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadConfig {
    /// Fuente que se guarda con cada vela.
    pub source: String,
    /// Pausa entre dos pedidos de historia.
    pub pause: std::time::Duration,
    /// Tope de pedidos de velas por corrida.
    pub max_requests: usize,
    /// Si no hay nada guardado de la serie y no se pide otra fecha, desde cuántos días atrás.
    pub backfill: TimeDelta,
    pub window_1m: TimeDelta,
    pub window_15m: TimeDelta,
    pub window_1h: TimeDelta,
    /// Atraso de los datos de IBKR sin suscripción (#563): la última vela de una respuesta se
    /// guarda solo si cerró hace al menos esto.
    pub data_delay: TimeDelta,
}

impl LoadConfig {
    /// Ventana de un pedido para ese tamaño. `None`: ese tamaño no se guarda (diarias).
    pub fn window(&self, size: BarSize) -> Option<TimeDelta> {
        match size {
            BarSize::M1 => Some(self.window_1m),
            BarSize::M15 => Some(self.window_15m),
            BarSize::H1 => Some(self.window_1h),
            BarSize::D1 => None,
        }
    }
}

/// Por qué no se carga una serie.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LoadRefusal {
    #[error("el futuro continuo de IBKR está ajustado por vencimiento (repinta): no se guarda")]
    ContinuousFuture,
    #[error("las diarias de IBKR vienen fechadas por sesión, sin hora: no se guardan")]
    Daily,
}

/// Se puede guardar este tipo de contrato y tamaño.
pub fn check_storable(kind: ContractKind, size: BarSize) -> Result<(), LoadRefusal> {
    if kind == ContractKind::ContinuousFuture {
        return Err(LoadRefusal::ContinuousFuture);
    }
    if size == BarSize::D1 {
        return Err(LoadRefusal::Daily);
    }
    Ok(())
}

/// Inicio de la vela en curso a `now`: las velas que empiezan antes ya cerraron.
pub fn closed_until(now: DateTime<Utc>, size: BarSize) -> DateTime<Utc> {
    now.duration_trunc(size.duration()).unwrap_or(now)
}

/// Cuántas velas del principio de `bars` (de una respuesta de IBKR, en orden) ya están
/// **completas** a `now` y se pueden guardar (#563).
///
/// La hora del reloj no alcanza: sin suscripción los datos de IBKR llegan con unos 10 minutos de
/// atraso, y la última vela que devuelve puede tener solo parte de sus datos aunque su hora de
/// cierre ya pasó. Una vela está completa si:
/// - cerró según el reloj (`inicio + tamaño <= now`), y
/// - IBKR devolvió una vela **después** de ella (sus datos ya pasaron su cierre), o, si es la
///   última, cerró hace al menos `delay` (cota para fin de sesión o mercado sin operaciones).
///
/// Corta en la primera que no cumple: nunca deja un hueco (la carga sigue desde la última guardada).
pub fn complete_prefix(
    bars: &[RawBar],
    size: BarSize,
    now: DateTime<Utc>,
    delay: TimeDelta,
) -> usize {
    bars.iter()
        .enumerate()
        .take_while(|(i, b)| {
            let end = b.start + size.duration();
            let has_later = i + 1 < bars.len();
            end <= now && (has_later || end <= now - delay)
        })
        .count()
}

/// Precios de una vela.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ohlc {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
}

/// La vela que forman las de 1 minuto `minutes` (en orden): apertura de la primera, máximo, mínimo
/// y cierre de la última. `None` si no hay ninguna.
pub fn aggregate(minutes: &[(DateTime<Utc>, Ohlc)]) -> Option<Ohlc> {
    let (_, first) = minutes.first()?;
    let (_, last) = minutes.last()?;
    Some(Ohlc {
        open: first.open,
        high: minutes.iter().map(|(_, m)| m.high).max()?,
        low: minutes.iter().map(|(_, m)| m.low).min()?,
        close: last.close,
    })
}

/// Resultado de comparar una vela de 15m o 1h con sus velas de 1m guardadas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coherence {
    /// Coincide con el agregado de las de 1m.
    Matches,
    /// No coincide: no se guarda.
    Differs { expected: Ohlc },
    /// No hay velas de 1m que cubran todo el período: no se puede revisar.
    NotChecked,
}

/// Qué precios se comparan con el agregado de las de 1m.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compare {
    /// Los cuatro (diagnóstico).
    All,
    /// Máximo, mínimo y cierre: lo que deja mal una vela con datos a medias (#563).
    HighLowClose,
    /// Solo el cierre.
    Close,
}

impl Compare {
    /// Qué compara `ibkr_load` antes de guardar. Futuros y divisas: máximo, mínimo y cierre. Acciones
    /// y ETF: solo el cierre, porque fuera del horario regular IBKR arma cada tamaño con otra
    /// operación de apertura (medido en QQQ, #563: 89 velas de 15m y 1h, todas fuera del horario
    /// regular, siempre distinta la apertura —y el máximo o mínimo cuando esa operación era el
    /// extremo—, el cierre siempre igual). El atraso de #563 siempre cambia el cierre.
    pub fn before_storing(kind: ContractKind) -> Self {
        match kind {
            ContractKind::Stock => Self::Close,
            _ => Self::HighLowClose,
        }
    }

    fn same(self, a: &Ohlc, b: &Ohlc) -> bool {
        let hlc = a.close == b.close && a.high == b.high && a.low == b.low;
        match self {
            Self::All => hlc && a.open == b.open,
            Self::HighLowClose => hlc,
            Self::Close => a.close == b.close,
        }
    }
}

/// Compara la vela `bar` (inicio `start`, tamaño `size`) con las velas de 1m guardadas de la misma
/// serie. `minutes`: las de 1m del período, en orden (las de afuera se ignoran). `last_minute`: el
/// inicio de la última de 1m guardada de la serie. Solo se revisa si las de 1m llegan **después**
/// del período (si no, podrían faltar las del final). Un minuto sin operaciones no tiene vela y no
/// cambia el agregado.
pub fn check_against_minutes(
    start: DateTime<Utc>,
    size: BarSize,
    bar: Ohlc,
    minutes: &[(DateTime<Utc>, Ohlc)],
    last_minute: Option<DateTime<Utc>>,
    compare: Compare,
) -> Coherence {
    let end = start + size.duration();
    if size == BarSize::M1 || last_minute.is_none_or(|l| l < end) {
        return Coherence::NotChecked;
    }
    let inside: Vec<(DateTime<Utc>, Ohlc)> = minutes
        .iter()
        .filter(|(t, _)| start <= *t && *t < end)
        .copied()
        .collect();
    match aggregate(&inside) {
        None => Coherence::NotChecked,
        Some(expected) if compare.same(&expected, &bar) => Coherence::Matches,
        Some(expected) => Coherence::Differs { expected },
    }
}

/// Desde dónde seguir: después de la última guardada; si no hay, desde `default_from`. Nunca
/// antes de la primera vela que tiene IBKR (`head`), redondeado al tamaño.
pub fn resume_from(
    last_stored: Option<DateTime<Utc>>,
    default_from: DateTime<Utc>,
    head: Option<DateTime<Utc>>,
    size: BarSize,
) -> DateTime<Utc> {
    let from = match last_stored {
        Some(last) => last + size.duration(),
        None => default_from,
    };
    let from = head.map_or(from, |h| from.max(h));
    from.duration_trunc(size.duration()).unwrap_or(from)
}

/// Ventanas `[desde, hasta)` consecutivas de `window` que cubren `[from, to)`, como mucho `max`.
pub fn windows(
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    window: TimeDelta,
    max: usize,
) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut out = Vec::new();
    let mut a = from;
    while a < to && out.len() < max && window > TimeDelta::zero() {
        let b = (a + window).min(to);
        out.push((a, b));
        a = b;
    }
    out
}

/// Qué hacer con el error de un pedido de velas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnError {
    /// IBKR no tiene velas en esa ventana (fin de semana, contrato que todavía no se operaba):
    /// se sigue con la siguiente.
    Empty,
    /// Violación de ritmo (pacing): se para la corrida.
    Pacing,
    /// Cualquier otro: se para la corrida.
    Stop,
}

pub fn classify(e: &SessionError) -> OnError {
    match e {
        SessionError::Rejected(ApiError::Rejected(m)) => {
            let m = m.to_ascii_lowercase();
            if m.contains("pacing") {
                OnError::Pacing
            } else if m.contains("returned no data") {
                OnError::Empty
            } else {
                OnError::Stop
            }
        }
        _ => OnError::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rust_decimal::dec;

    fn t(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 25, h, m, 0).unwrap()
    }

    fn bar(start: DateTime<Utc>, o: Decimal, h: Decimal, l: Decimal, c: Decimal) -> RawBar {
        RawBar {
            start,
            open: o,
            high: h,
            low: l,
            close: c,
            volume: Some(dec!(1)),
        }
    }

    fn flat(start: DateTime<Utc>) -> RawBar {
        bar(start, dec!(10), dec!(11), dec!(9), dec!(10))
    }

    const DELAY: TimeDelta = TimeDelta::minutes(15);

    /// Caso real de #563: carga de MNQ 15m a las 02:16 del 25-11-2026. IBKR (datos con ~10 min de
    /// atraso) devolvió la vela de las 02:00 con datos solo hasta ~02:10 (L 26028, C 26035.25). El
    /// reloj decía que había cerrado (02:15 <= 02:16) y se guardó así. Ahora no se guarda: es la
    /// última de la respuesta y cerró hace menos del atraso. La de las 01:45 sí (hay una después).
    #[test]
    fn regression_563_partial_last_bar_is_not_stored() {
        let got = vec![
            flat(t(1, 30)),
            flat(t(1, 45)),
            bar(
                t(2, 0),
                dec!(26068.5),
                dec!(26093.75),
                dec!(26028),
                dec!(26035.25),
            ),
        ];
        assert_eq!(complete_prefix(&got, BarSize::M15, t(2, 16), DELAY), 2);
        // Corrida siguiente (02:31): IBKR ya devuelve la de las 02:15 después; la de las 02:00
        // entra completa y la de las 02:15 espera.
        let next = vec![flat(t(2, 0)), flat(t(2, 15))];
        assert_eq!(complete_prefix(&next, BarSize::M15, t(2, 31), DELAY), 1);
    }

    #[test]
    fn last_bar_waits_the_delay_and_earlier_ones_need_a_later_bar() {
        let got = vec![flat(t(1, 0)), flat(t(2, 0))];
        // 1h: la de las 02:00 cierra a las 03:00; con 15 min de atraso entra desde las 03:15.
        assert_eq!(complete_prefix(&got, BarSize::H1, t(3, 14), DELAY), 1);
        assert_eq!(complete_prefix(&got, BarSize::H1, t(3, 15), DELAY), 2);
        // Fin de sesión o mercado sin operaciones: la última entra por el atraso.
        assert_eq!(
            complete_prefix(&[flat(t(1, 0))], BarSize::H1, t(5, 0), DELAY),
            1
        );
        // También en 1m: la última de 1m puede estar a medias.
        let m = vec![flat(t(2, 0)), flat(t(2, 1))];
        assert_eq!(complete_prefix(&m, BarSize::M1, t(2, 5), DELAY), 1);
    }

    #[test]
    fn complete_prefix_edge_cases() {
        assert_eq!(complete_prefix(&[], BarSize::M15, t(2, 0), DELAY), 0);
        // La vela en curso según el reloj nunca entra, aunque haya otra después (dato raro).
        let odd = vec![flat(t(2, 0)), flat(t(2, 15))];
        assert_eq!(complete_prefix(&odd, BarSize::M15, t(2, 10), DELAY), 0);
        // Hueco (fin de semana): la de antes del hueco tiene una después, entra.
        let gap = vec![flat(t(1, 0)), flat(t(9, 0))];
        assert_eq!(complete_prefix(&gap, BarSize::H1, t(9, 30), DELAY), 1);
        // Atraso cero: solo cuenta el reloj.
        assert_eq!(
            complete_prefix(&[flat(t(1, 0))], BarSize::H1, t(2, 0), TimeDelta::zero()),
            1
        );
    }

    fn o(open: Decimal, high: Decimal, low: Decimal, close: Decimal) -> Ohlc {
        Ohlc {
            open,
            high,
            low,
            close,
        }
    }

    /// Las 1m del caso real (resumidas): el agregado de 02:00–02:14 da L 26022.75 C 26050.
    fn minutes_563() -> Vec<(DateTime<Utc>, Ohlc)> {
        vec![
            (
                t(2, 0),
                o(dec!(26068.5), dec!(26093.75), dec!(26060), dec!(26062)),
            ),
            (
                t(2, 5),
                o(dec!(26062), dec!(26070), dec!(26030), dec!(26040)),
            ),
            (
                t(2, 10),
                o(dec!(26040), dec!(26045), dec!(26028), dec!(26035.25)),
            ),
            (
                t(2, 12),
                o(dec!(26035.25), dec!(26040), dec!(26022.75), dec!(26030)),
            ),
            (
                t(2, 14),
                o(dec!(26030), dec!(26055), dec!(26029), dec!(26050)),
            ),
            // Fuera del período: no cuenta.
            (
                t(2, 15),
                o(dec!(26050), dec!(26200), dec!(26000), dec!(26100)),
            ),
        ]
    }

    #[test]
    fn regression_563_stored_bar_differs_from_its_minutes() {
        let stored = o(dec!(26068.5), dec!(26093.75), dec!(26028), dec!(26035.25));
        let r = check_against_minutes(
            t(2, 0),
            BarSize::M15,
            stored,
            &minutes_563(),
            Some(t(2, 15)),
            Compare::All,
        );
        assert_eq!(
            r,
            Coherence::Differs {
                expected: o(dec!(26068.5), dec!(26093.75), dec!(26022.75), dec!(26050))
            }
        );
        let good = o(dec!(26068.5), dec!(26093.75), dec!(26022.75), dec!(26050));
        assert_eq!(
            check_against_minutes(
                t(2, 0),
                BarSize::M15,
                good,
                &minutes_563(),
                Some(t(2, 15)),
                Compare::All
            ),
            Coherence::Matches
        );
    }

    #[test]
    fn coherence_is_not_checked_without_full_minute_coverage() {
        let any = o(dec!(1), dec!(2), dec!(1), dec!(1));
        let m = minutes_563();
        // Las 1m guardadas no pasan del final del período: podrían faltar las últimas.
        assert_eq!(
            check_against_minutes(t(2, 0), BarSize::M15, any, &m, Some(t(2, 14)), Compare::All),
            Coherence::NotChecked
        );
        assert_eq!(
            check_against_minutes(t(2, 0), BarSize::M15, any, &m, None, Compare::All),
            Coherence::NotChecked
        );
        // Ninguna 1m dentro del período (sin operaciones): no se puede revisar.
        assert_eq!(
            check_against_minutes(t(3, 0), BarSize::M15, any, &m, Some(t(4, 0)), Compare::All),
            Coherence::NotChecked
        );
        // Las de 1m no se revisan contra sí mismas.
        assert_eq!(
            check_against_minutes(t(2, 0), BarSize::M1, any, &m, Some(t(4, 0)), Compare::All),
            Coherence::NotChecked
        );
        assert_eq!(aggregate(&[]), None);
    }

    #[test]
    fn what_is_compared_before_storing() {
        let m = minutes_563();
        let good = o(dec!(26068.5), dec!(26093.75), dec!(26022.75), dec!(26050));
        let other_open = Ohlc {
            open: dec!(26070),
            ..good
        };
        let check =
            |bar, c| check_against_minutes(t(2, 0), BarSize::M15, bar, &m, Some(t(2, 15)), c);
        // Solo la apertura distinta (lo que pasa con QQQ fuera del horario regular).
        assert!(matches!(
            check(other_open, Compare::All),
            Coherence::Differs { .. }
        ));
        assert_eq!(check(other_open, Compare::HighLowClose), Coherence::Matches);
        // La vela a medias de #563 no pasa con ninguna regla: el cierre es otro.
        let partial = o(dec!(26068.5), dec!(26093.75), dec!(26028), dec!(26035.25));
        for c in [Compare::All, Compare::HighLowClose, Compare::Close] {
            assert!(matches!(check(partial, c), Coherence::Differs { .. }));
        }
        let other_low = Ohlc {
            low: dec!(26000),
            ..good
        };
        assert!(matches!(
            check(other_low, Compare::HighLowClose),
            Coherence::Differs { .. }
        ));
        assert_eq!(check(other_low, Compare::Close), Coherence::Matches);
        assert_eq!(Compare::before_storing(ContractKind::Stock), Compare::Close);
        assert_eq!(
            Compare::before_storing(ContractKind::Future),
            Compare::HighLowClose
        );
        assert_eq!(
            Compare::before_storing(ContractKind::Forex),
            Compare::HighLowClose
        );
    }

    #[test]
    fn only_closed_bars_are_planned() {
        assert_eq!(closed_until(t(10, 37), BarSize::H1), t(10, 0));
        assert_eq!(closed_until(t(10, 37), BarSize::M15), t(10, 30));
        assert_eq!(closed_until(t(10, 37), BarSize::M1), t(10, 37));
    }

    #[test]
    fn resume_after_the_last_stored_never_before_head() {
        let def = t(0, 0);
        assert_eq!(resume_from(Some(t(5, 0)), def, None, BarSize::H1), t(6, 0));
        assert_eq!(resume_from(None, def, None, BarSize::H1), def);
        assert_eq!(resume_from(None, def, Some(t(3, 20)), BarSize::H1), t(3, 0));
        assert_eq!(resume_from(None, t(2, 7), None, BarSize::M15), t(2, 0));
    }

    #[test]
    fn windows_cover_the_range_up_to_the_cap() {
        let w = windows(t(0, 0), t(5, 0), TimeDelta::hours(2), 10);
        assert_eq!(
            w,
            vec![(t(0, 0), t(2, 0)), (t(2, 0), t(4, 0)), (t(4, 0), t(5, 0))]
        );
        assert_eq!(windows(t(0, 0), t(5, 0), TimeDelta::hours(2), 1).len(), 1);
        assert!(windows(t(5, 0), t(5, 0), TimeDelta::hours(2), 10).is_empty());
    }

    #[test]
    fn continuous_and_daily_are_never_stored() {
        assert_eq!(
            check_storable(ContractKind::ContinuousFuture, BarSize::H1),
            Err(LoadRefusal::ContinuousFuture)
        );
        assert_eq!(
            check_storable(ContractKind::Future, BarSize::D1),
            Err(LoadRefusal::Daily)
        );
        assert!(check_storable(ContractKind::Stock, BarSize::M1).is_ok());
    }

    #[test]
    fn pacing_stops_and_no_data_is_empty() {
        let rej = |m: &str| SessionError::Rejected(ApiError::Rejected(m.into()));
        assert_eq!(
            classify(&rej(
                "[162] Historical Market Data Service error message:Historical data request pacing violation"
            )),
            OnError::Pacing
        );
        assert_eq!(
            classify(&rej(
                "[162] Historical Market Data Service error message:HMDS query returned no data: MNQZ6@CME Trades"
            )),
            OnError::Empty
        );
        assert_eq!(
            classify(&rej("[162] No market data permissions for IDEALPRO CASH")),
            OnError::Stop
        );
    }
}
