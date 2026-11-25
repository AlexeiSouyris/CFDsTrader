//! Aviso de carga trabada (#607): cuándo la carga de velas de IBKR (`ibkr_load`) lleva demasiado
//! tiempo sin que IBKR responda.
//!
//! El 25-11-2026, después del reinicio nocturno de IBKR, el servidor de historia de IB Gateway
//! quedó apagado y **todos** los pedidos fallaron con "timed out" durante 5 horas, sin aviso.
//!
//! - Cada corrida de una serie (instrumento + tamaño) termina en [`RunOutcome::Answered`] (IBKR
//!   respondió, **con o sin velas**: mercado cerrado, pausa de CME o fin de semana dan 0 velas sin
//!   error, y eso es normal) o [`RunOutcome::Failed`] (no se pudo conectar, se cortó la conexión o
//!   el pedido no terminó a tiempo). Lo que falla por la base o la configuración no cuenta.
//! - La racha de fallos de cada serie se guarda en un archivo JSON chico ([`HealthState`]); la
//!   cuenta [`step`], una función pura.
//! - Hay corte cuando una serie lleva al menos `min_failures` corridas fallidas seguidas **y**
//!   `stall_minutes` desde la primera. El aviso sale **una vez** al empezar
//!   ([`Transition::Stalled`]) y **una vez** al volver ([`Transition::Recovered`]): cuando IBKR
//!   responde y ninguna serie sigue trabada.
//! - Los umbrales van en `[load_health]` de `config/ibkr.toml`.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, TimeDelta, Utc};
use serde::{Deserialize, Serialize};

use crate::client::ConnectError;
use crate::session::{ApiError, SessionError};

/// `[load_health]` de `config/ibkr.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthConfig {
    /// Corridas fallidas seguidas de una serie para avisar.
    pub min_failures: u32,
    /// Tiempo mínimo desde el primer fallo de la racha para avisar.
    pub stall: TimeDelta,
    /// Una racha cuya última corrida es más vieja que esto se olvida (una serie que ya no corre
    /// no deja el aviso prendido para siempre).
    pub forget_after: TimeDelta,
    /// Notificación de macOS (`osascript`) al empezar y al terminar un corte.
    pub notify_macos: bool,
    /// Archivo de la racha, relativo a la raíz del repo.
    pub state_file: String,
}

/// Cómo terminó una corrida, mirando solo a IBKR.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    /// IBKR respondió (con velas, sin velas, con un rechazo o un aviso de ritmo).
    Answered,
    /// IBKR no respondió: conexión, corte o tiempo agotado. El texto es el error.
    Failed(String),
}

/// Si un texto de error de IBKR es de "no responde": tiempo agotado o conexión.
fn unreachable_text(m: &str) -> bool {
    let m = m.to_ascii_lowercase();
    [
        "timed out",
        "timeout",
        "connection",
        "not connected",
        "farm",
    ]
    .iter()
    .any(|k| m.contains(k))
}

/// El resultado de un error al conectarse. `None`: no es culpa de IBKR (configuración, cuenta
/// paper faltante en `.env`): no cuenta para la racha.
pub fn connect_outcome(e: &ConnectError) -> Option<RunOutcome> {
    match e {
        ConnectError::Connect { .. } | ConnectError::Tripped(_) => {
            Some(RunOutcome::Failed(e.to_string()))
        }
        ConnectError::PaperAccount(_)
        | ConnectError::Config(_)
        | ConnectError::RecordingEnabled(_) => None,
    }
}

/// El resultado de un error de un pedido. Un rechazo de IBKR (sin datos, sin permiso, contrato
/// desconocido, ritmo) es una respuesta; tiempo agotado o conexión perdida, un fallo. `None`: el
/// instrumento no está configurado (no es culpa de IBKR).
pub fn session_outcome(e: &SessionError) -> Option<RunOutcome> {
    let failed = || Some(RunOutcome::Failed(e.to_string()));
    match e {
        SessionError::KillSwitch(_) | SessionError::Rejected(ApiError::ConnectionLost(_)) => {
            failed()
        }
        SessionError::Rejected(ApiError::Rejected(m)) if unreachable_text(m) => failed(),
        SessionError::Rejected(_) => Some(RunOutcome::Answered),
        SessionError::UnknownInstrument(_) => None,
    }
}

/// La racha de fallos de una serie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Streak {
    pub failures: u32,
    /// Primer fallo de la racha (UTC).
    pub since: DateTime<Utc>,
    /// Último fallo (UTC).
    pub last: DateTime<Utc>,
    /// Último error.
    pub error: String,
}

/// Lo que se guarda entre corridas.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthState {
    /// Rachas abiertas por serie ("MNQ 1m"). Una serie que respondió no está.
    #[serde(default)]
    pub streaks: BTreeMap<String, Streak>,
    /// Desde cuándo hay un corte avisado (primer fallo); `None` si no hay aviso abierto.
    #[serde(default)]
    pub alerted_since: Option<DateTime<Utc>>,
}

/// Qué hay que avisar después de una corrida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Transition {
    None,
    /// Empezó un corte: avisar una vez.
    Stalled {
        since: DateTime<Utc>,
        /// Series trabadas, en orden.
        series: Vec<String>,
        failures: u32,
        error: String,
    },
    /// IBKR volvió a responder: avisar una vez.
    Recovered {
        since: DateTime<Utc>,
    },
}

fn stalled(s: &Streak, now: DateTime<Utc>, cfg: &HealthConfig) -> bool {
    s.failures >= cfg.min_failures && now - s.since >= cfg.stall
}

/// Suma una corrida de `series` a la racha y dice si hay que avisar. Pura: mismo estado, mismo
/// resultado.
pub fn step(
    state: &HealthState,
    series: &str,
    outcome: &RunOutcome,
    now: DateTime<Utc>,
    cfg: &HealthConfig,
) -> (HealthState, Transition) {
    let mut next = state.clone();
    next.streaks.retain(|_, s| now - s.last <= cfg.forget_after);
    match outcome {
        RunOutcome::Answered => {
            next.streaks.remove(series);
        }
        RunOutcome::Failed(error) => {
            let s = next
                .streaks
                .entry(series.to_owned())
                .or_insert_with(|| Streak {
                    failures: 0,
                    since: now,
                    last: now,
                    error: String::new(),
                });
            s.failures = s.failures.saturating_add(1);
            s.last = now;
            s.error.clone_from(error);
        }
    }
    let stuck: Vec<(&String, &Streak)> = next
        .streaks
        .iter()
        .filter(|(_, s)| stalled(s, now, cfg))
        .collect();
    let first = stuck.iter().map(|(_, s)| s.since).min();
    let worst = stuck.iter().map(|(_, s)| *s).max_by_key(|s| s.failures);
    let transition = match (next.alerted_since, first, worst) {
        (None, Some(since), Some(worst)) => {
            let t = Transition::Stalled {
                since,
                series: stuck.iter().map(|(k, _)| (*k).clone()).collect(),
                failures: worst.failures,
                error: worst.error.clone(),
            };
            next.alerted_since = Some(since);
            t
        }
        (Some(since), None, _) if *outcome == RunOutcome::Answered => {
            next.alerted_since = None;
            Transition::Recovered { since }
        }
        _ => Transition::None,
    };
    (next, transition)
}

/// "35 min", "1 h", "5 h 10 min".
pub fn duration_text(d: TimeDelta) -> String {
    let m = d.num_minutes().max(0);
    match (m / 60, m % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// El texto del aviso de corte, para la app y la notificación.
pub fn stalled_message(since: DateTime<Utc>, now: DateTime<Utc>) -> String {
    format!(
        "IBKR no responde hace {}: reiniciá IB Gateway.",
        duration_text(now - since)
    )
}

/// El texto de cuando vuelve a responder.
pub fn recovered_message(since: DateTime<Utc>, now: DateTime<Utc>) -> String {
    format!(
        "IBKR volvió a responder después de {} sin datos.",
        duration_text(now - since)
    )
}

/// Lee el estado. Sin archivo, vacío. Si no se puede leer o está roto, vacío y el motivo (la racha
/// vuelve a empezar: a lo sumo se avisa un poco más tarde).
pub fn read_state(path: &Path) -> (HealthState, Option<String>) {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (HealthState::default(), None),
        Err(e) => (HealthState::default(), Some(e.to_string())),
        Ok(text) => match serde_json::from_str(&text) {
            Ok(s) => (s, None),
            Err(e) => (HealthState::default(), Some(e.to_string())),
        },
    }
}

/// Guarda el estado entero: primero en un archivo de al lado y después lo renombra, así nunca
/// queda a medias.
pub fn write_state(path: &Path, state: &HealthState) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(state).map_err(std::io::Error::other)?;
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn cfg() -> HealthConfig {
        HealthConfig {
            min_failures: 6,
            stall: TimeDelta::minutes(30),
            forget_after: TimeDelta::hours(6),
            notify_macos: false,
            state_file: "logs/ibkr_load_health.json".into(),
        }
    }

    fn t(h: u32, m: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 25, h, m, 0).unwrap()
    }

    fn fail() -> RunOutcome {
        RunOutcome::Failed("IBKR rechazó el pedido: timed out before the request ended".into())
    }

    /// Corre `series` cada 5 minutos desde `from`, `n` veces, y junta los avisos.
    fn every_5_min(
        mut s: HealthState,
        series: &[&str],
        outcome: &RunOutcome,
        from: DateTime<Utc>,
        n: i64,
    ) -> (HealthState, Vec<(DateTime<Utc>, Transition)>) {
        let mut out = Vec::new();
        for k in 0..n {
            let now = from + TimeDelta::minutes(5 * k);
            for name in series {
                let (next, tr) = step(&s, name, outcome, now, &cfg());
                s = next;
                if tr != Transition::None {
                    out.push((now, tr));
                }
            }
        }
        (s, out)
    }

    const ALL: [&str; 9] = [
        "MNQ 1m", "MNQ 15m", "MNQ 1h", "MES 1m", "MES 15m", "MES 1h", "QQQ 1m", "QQQ 15m", "QQQ 1h",
    ];

    #[test]
    fn failures_alert_once_at_the_threshold() {
        let (s, alerts) = every_5_min(HealthState::default(), &ALL, &fail(), t(5, 35), 20);
        assert_eq!(alerts.len(), 1, "un solo aviso en 100 minutos de fallos");
        let (at, tr) = &alerts[0];
        // 6 fallos seguidos y 30 minutos: la 7.ª corrida (05:35 + 30 min).
        assert_eq!(*at, t(6, 5));
        match tr {
            Transition::Stalled {
                since,
                series,
                failures,
                error,
            } => {
                assert_eq!(*since, t(5, 35));
                // Todas fallan desde 05:35: a las 06:05 todas llegaron a 30 minutos (las demás
                // con 6 fallos, MNQ 1m, la primera del cron, con 7).
                let mut all: Vec<String> = ALL.iter().map(|s| (*s).to_owned()).collect();
                all.sort();
                assert_eq!(series, &all);
                assert_eq!(*failures, 7);
                assert!(error.contains("timed out"));
            }
            other => panic!("se esperaba Stalled: {other:?}"),
        }
        assert_eq!(s.alerted_since, Some(t(5, 35)));
    }

    #[test]
    fn below_the_threshold_does_not_alert() {
        // 5 fallos en 20 minutos (un reinicio nocturno corto) y después responde.
        let (s, alerts) = every_5_min(HealthState::default(), &ALL, &fail(), t(5, 35), 5);
        assert!(alerts.is_empty());
        let (s, alerts) = every_5_min(s, &ALL, &RunOutcome::Answered, t(6, 0), 1);
        assert!(
            alerts.is_empty(),
            "no avisa que volvió si no avisó el corte"
        );
        assert_eq!(s, HealthState::default());
    }

    #[test]
    fn many_failures_in_little_time_do_not_alert() {
        // 6 fallos seguidos, pero en 10 minutos (corridas manuales): falta tiempo.
        let mut s = HealthState::default();
        for k in 0..6 {
            let (next, tr) = step(
                &s,
                "MNQ 1m",
                &fail(),
                t(9, 0) + TimeDelta::minutes(2 * k),
                &cfg(),
            );
            assert_eq!(tr, Transition::None);
            s = next;
        }
        assert_eq!(s.streaks["MNQ 1m"].failures, 6);
    }

    #[test]
    fn an_answer_resets_the_streak() {
        let (s, _) = every_5_min(HealthState::default(), &["MNQ 1m"], &fail(), t(5, 35), 5);
        let (s, tr) = step(&s, "MNQ 1m", &RunOutcome::Answered, t(6, 0), &cfg());
        assert_eq!(tr, Transition::None);
        assert!(s.streaks.is_empty());
        // Vuelve a fallar: la racha empieza de cero, no avisa a los 30 min del primer fallo viejo.
        let (_, alerts) = every_5_min(s, &["MNQ 1m"], &fail(), t(6, 5), 6);
        assert!(alerts.is_empty());
    }

    #[test]
    fn recovery_is_announced_once_when_every_series_answers() {
        let (s, _) = every_5_min(HealthState::default(), &ALL, &fail(), t(5, 35), 60);
        assert_eq!(s.alerted_since, Some(t(5, 35)));
        // Se reinició IB Gateway a las 10:35: la primera serie responde, las demás todavía no
        // corrieron: no se avisa hasta que ninguna siga trabada.
        let (s1, tr) = step(&s, "MNQ 1m", &RunOutcome::Answered, t(10, 35), &cfg());
        assert_eq!(tr, Transition::None);
        let (s2, alerts) = every_5_min(s1, &ALL[1..], &RunOutcome::Answered, t(10, 35), 1);
        assert_eq!(
            alerts,
            vec![(t(10, 35), Transition::Recovered { since: t(5, 35) })]
        );
        assert_eq!(s2, HealthState::default());
        // Y sigue respondiendo: nada más.
        let (_, alerts) = every_5_min(s2, &ALL, &RunOutcome::Answered, t(10, 40), 12);
        assert!(alerts.is_empty());
    }

    #[test]
    fn closed_market_with_zero_bars_never_alerts() {
        // Fin de semana: IBKR responde sin velas (o "returned no data", que es una respuesta).
        let (s, alerts) = every_5_min(
            HealthState::default(),
            &ALL,
            &RunOutcome::Answered,
            t(0, 0),
            12 * 48,
        );
        assert!(alerts.is_empty());
        assert_eq!(s, HealthState::default());
        let no_data = SessionError::Rejected(ApiError::Rejected(
            "HMDS query returned no data: MNQ".into(),
        ));
        assert_eq!(session_outcome(&no_data), Some(RunOutcome::Answered));
    }

    #[test]
    fn one_stuck_series_alerts_even_if_others_answer() {
        let mut s = HealthState::default();
        let mut alerts = Vec::new();
        for k in 0..8 {
            let now = t(14, 0) + TimeDelta::minutes(5 * k);
            for name in ALL {
                let o = if name == "QQQ 1h" {
                    fail()
                } else {
                    RunOutcome::Answered
                };
                let (next, tr) = step(&s, name, &o, now, &cfg());
                s = next;
                if tr != Transition::None {
                    alerts.push(tr);
                }
            }
        }
        assert_eq!(alerts.len(), 1);
        assert!(
            matches!(&alerts[0], Transition::Stalled { series, .. } if series == &vec!["QQQ 1h".to_owned()])
        );
        // Otras series que responden no cierran el aviso mientras QQQ 1h siga trabada.
        let (s, tr) = step(&s, "MNQ 1m", &RunOutcome::Answered, t(15, 0), &cfg());
        assert_eq!(tr, Transition::None);
        let (_, tr) = step(&s, "QQQ 1h", &RunOutcome::Answered, t(15, 0), &cfg());
        assert_eq!(tr, Transition::Recovered { since: t(14, 0) });
    }

    #[test]
    fn old_streaks_are_forgotten() {
        let (s, _) = every_5_min(HealthState::default(), &["EURUSD 1h"], &fail(), t(1, 0), 10);
        assert!(s.alerted_since.is_some());
        // La serie dejó de correr; 7 horas después otra responde: la racha vieja se olvida, IBKR
        // responde y el aviso se cierra.
        let (s, tr) = step(&s, "MNQ 1m", &RunOutcome::Answered, t(9, 0), &cfg());
        assert_eq!(tr, Transition::Recovered { since: t(1, 0) });
        assert_eq!(s, HealthState::default());
    }

    #[test]
    fn classifies_errors_from_ibkr() {
        let timeout = SessionError::Rejected(ApiError::Rejected(
            "timed out before the request ended".into(),
        ));
        assert!(matches!(
            session_outcome(&timeout),
            Some(RunOutcome::Failed(_))
        ));
        let lost = SessionError::Rejected(ApiError::ConnectionLost("eof".into()));
        assert!(matches!(
            session_outcome(&lost),
            Some(RunOutcome::Failed(_))
        ));
        let pacing = SessionError::Rejected(ApiError::Rejected(
            "Historical Market Data Service error message:pacing violation".into(),
        ));
        assert_eq!(session_outcome(&pacing), Some(RunOutcome::Answered));
        let perm = SessionError::Rejected(ApiError::NoPermission("10089".into()));
        assert_eq!(session_outcome(&perm), Some(RunOutcome::Answered));
        let unknown = SessionError::UnknownInstrument("XYZ".parse().unwrap());
        assert_eq!(session_outcome(&unknown), None);
        let refused = ConnectError::Connect {
            address: "127.0.0.1:4002".into(),
            reason: "connection refused".into(),
        };
        assert!(matches!(
            connect_outcome(&refused),
            Some(RunOutcome::Failed(_))
        ));
        let no_env = ConnectError::RecordingEnabled("IBAPI_RECORDING_DIR");
        assert_eq!(connect_outcome(&no_env), None);
    }

    #[test]
    fn messages_in_simple_words() {
        assert_eq!(
            stalled_message(t(5, 35), t(6, 10)),
            "IBKR no responde hace 35 min: reiniciá IB Gateway."
        );
        assert_eq!(
            recovered_message(t(5, 35), t(10, 45)),
            "IBKR volvió a responder después de 5 h 10 min sin datos."
        );
        assert_eq!(duration_text(TimeDelta::hours(2)), "2 h");
    }

    #[test]
    fn state_survives_a_round_trip_and_bad_files_start_fresh() {
        let dir = std::env::temp_dir().join(format!("cfds-health-{}", std::process::id()));
        let path = dir.join("h.json");
        let (s, _) = every_5_min(HealthState::default(), &ALL, &fail(), t(5, 35), 8);
        write_state(&path, &s).unwrap();
        assert_eq!(read_state(&path), (s, None));
        std::fs::write(&path, "{ roto").unwrap();
        let (back, why) = read_state(&path);
        assert_eq!(back, HealthState::default());
        assert!(why.is_some());
        let (back, why) = read_state(&dir.join("no-existe.json"));
        assert_eq!((back, why), (HealthState::default(), None));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
