//! `bolsa`: precios y mini gráfico de velas de IBKR paper en la terminal, solo lectura (#541).
//!
//! ```text
//! cd engine && set -a && . ../.env && set +a
//! cargo run -q --release -p cfds-terminal --bin bolsa                 # pantalla completa (q para salir)
//! cargo run -q --release -p cfds-terminal --bin bolsa -- --once       # la tabla una vez, sin gráfico
//! cargo run -q --release -p cfds-terminal --bin bolsa -- --check-config
//! ```
//!
//! Antes: IB Gateway abierto con login paper hecho a mano y `IBKR_PAPER_ACCOUNT_ID` en `.env`. Lee
//! `config/terminal.toml`, `config/ibkr.toml` y `config/alerts.toml`. No guarda nada.
//! Teclas: ↑/↓ (o k/j, o 1–9) instrumento, ←/→ (o h/l) timeframe, q / Esc / Ctrl+C salir.
//! Sin colores con `NO_COLOR=1` o `--no-color`.
//!
//! Salida: 0 bien; 1 configuración inválida o no se pudo conectar; 2 kill switch (cuenta que no es
//! paper, desconexión o error de datos), con alerta al log (y a Telegram si está configurado).

use std::collections::HashMap;
use std::io::{IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;

use cfds_ibkr::{
    BarSize, ConnectError, ContractChoice, IbapiClient, IbkrConfig, RawBar, ReadOnlySession,
    SessionError, SourcedBars, Trip, connect,
};
use cfds_obs::{AlertConfig, Alerter, Logger, ReqwestPost, TelegramSink};
use cfds_terminal::quote::{day_stats, merge_minutes, minute_window_start, quote};
use cfds_terminal::schedule::{Periods, Scheduler, Task};
use cfds_terminal::view::{self, ChartView, Line, Link, Row, Screen, Tone};
use cfds_terminal::{TerminalConfig, config::MIN_GAP_SECS};
use chrono::{DateTime, TimeDelta, Utc};
use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, ClearType};
use crossterm::{cursor, execute, queue};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

const TERMINAL: &str = include_str!("../../../../../config/terminal.toml");
const IBKR: &str = include_str!("../../../../../config/ibkr.toml");
const ALERTS: &str = include_str!("../../../../../config/alerts.toml");

/// En `--once` hay 2 pedidos por instrumento y nada más: pausa corta entre ellos.
const ONCE_GAP_SECS: i64 = 3;

type Session = ReadOnlySession<IbapiClient>;

struct Inst {
    contract: Option<String>,
    /// Velas de 1 minuto cerradas de las últimas horas (para el último precio y el rango del día).
    minutes: Vec<RawBar>,
    daily: Vec<RawBar>,
    error: Option<String>,
    /// Corrimiento del día de sesión (según el tipo de contrato).
    offset: TimeDelta,
}

struct ChartData {
    contract: String,
    source: String,
    bars: Vec<RawBar>,
}

struct App {
    cfg: TerminalConfig,
    ids: Vec<cfds_core::InstrumentId>,
    insts: Vec<Inst>,
    charts: HashMap<(usize, BarSize), Result<ChartData, String>>,
    selected: usize,
    tf: usize,
    sched: Scheduler,
    trip: Option<Trip>,
    address: String,
}

fn short(e: &impl std::fmt::Display) -> String {
    let s = e.to_string();
    if s.chars().count() > 90 {
        format!("{}…", s.chars().take(89).collect::<String>())
    } else {
        s
    }
}

impl App {
    fn new(cfg: TerminalConfig, ibkr: &IbkrConfig) -> Self {
        let tf = cfg
            .timeframes
            .iter()
            .position(|t| *t == cfg.initial_timeframe)
            .unwrap_or(0);
        Self {
            ids: cfg.instruments.clone(),
            insts: cfg
                .instruments
                .iter()
                .map(|id| Inst {
                    contract: None,
                    minutes: Vec::new(),
                    daily: Vec::new(),
                    error: None,
                    // `check_against` ya verificó que cada instrumento está en config/ibkr.toml.
                    offset: ibkr
                        .instrument(id)
                        .map(|spec| cfg.session_offset(spec.kind))
                        .unwrap_or(cfg.other_session_offset),
                })
                .collect(),
            address: format!("{}:{}", ibkr.host, ibkr.port),
            charts: HashMap::new(),
            selected: 0,
            tf,
            sched: Scheduler::new(),
            trip: None,
            cfg,
        }
    }

    fn size(&self) -> BarSize {
        self.cfg.timeframes[self.tf]
    }

    fn periods(&self, once: bool) -> Periods {
        Periods {
            gap: if once {
                TimeDelta::seconds(ONCE_GAP_SECS)
            } else {
                self.cfg.min_request_gap
            },
            quote: self.cfg.quote_every,
            daily: self.cfg.daily_every,
            chart: self.cfg.chart_every,
        }
    }

    /// Ventana y tamaño de vela del pedido.
    fn plan(&self, task: Task, now: DateTime<Utc>) -> (usize, BarSize, DateTime<Utc>) {
        match task {
            Task::Quote(i) => (
                i,
                BarSize::M1,
                minute_window_start(
                    self.insts[i].minutes.last().map(|b| b.start),
                    now,
                    self.cfg.quote_lookback,
                ),
            ),
            Task::Daily(i) => (i, BarSize::D1, now - self.cfg.daily_lookback),
            Task::Chart(i, size) => (i, size, now - self.cfg.chart_window(size)),
        }
    }

    /// Guarda lo que llegó. Devuelve el motivo si se activó el kill switch.
    fn apply(&mut self, task: Task, got: Result<SourcedBars, SessionError>) -> Option<Trip> {
        let got = match got {
            Ok(g) => g,
            Err(SessionError::KillSwitch(trip)) => {
                self.trip = Some(trip.clone());
                return Some(trip);
            }
            Err(e) => {
                let msg = short(&e);
                match task {
                    Task::Quote(i) | Task::Daily(i) => self.insts[i].error = Some(msg),
                    Task::Chart(i, s) => {
                        self.charts.insert((i, s), Err(msg));
                    }
                }
                return None;
            }
        };
        match task {
            Task::Quote(i) => {
                let inst = &mut self.insts[i];
                inst.contract = Some(got.contract);
                inst.error = None;
                merge_minutes(
                    &mut inst.minutes,
                    got.bars,
                    Utc::now() - self.cfg.quote_lookback,
                );
            }
            Task::Daily(i) => {
                let inst = &mut self.insts[i];
                inst.contract = Some(got.contract);
                inst.error = None;
                inst.daily = got.bars;
            }
            Task::Chart(i, s) => {
                self.charts.insert(
                    (i, s),
                    Ok(ChartData {
                        contract: got.contract,
                        source: got.source,
                        bars: got.bars,
                    }),
                );
            }
        }
        None
    }

    fn rows(&self) -> Vec<Row> {
        self.ids
            .iter()
            .zip(&self.insts)
            .map(|(id, s)| Row {
                id: id.to_string(),
                contract: s.contract.clone(),
                quote: quote(s.minutes.last(), &s.daily, s.offset),
                day: day_stats(&s.minutes, s.offset),
                error: s.error.clone(),
            })
            .collect()
    }

    fn key(&mut self, code: KeyCode) {
        let n = self.ids.len();
        let m = self.cfg.timeframes.len();
        match code {
            KeyCode::Up | KeyCode::Char('k') => self.selected = (self.selected + n - 1) % n,
            KeyCode::Down | KeyCode::Char('j') => self.selected = (self.selected + 1) % n,
            KeyCode::Left | KeyCode::Char('h') => self.tf = (self.tf + m - 1) % m,
            KeyCode::Right | KeyCode::Char('l') | KeyCode::Tab => self.tf = (self.tf + 1) % m,
            KeyCode::Char(c) => {
                if let Some(d) = c.to_digit(10) {
                    let d = d as usize;
                    if (1..=n).contains(&d) {
                        self.selected = d - 1;
                    }
                }
            }
            _ => {}
        }
    }
}

async fn fetch(
    session: Arc<Session>,
    id: cfds_core::InstrumentId,
    size: BarSize,
    from: DateTime<Utc>,
) -> Result<SourcedBars, SessionError> {
    let now = Utc::now();
    session
        .closed_bars(&id, ContractChoice::Front, size, from, now, now)
        .await
}

fn alerter() -> Result<Alerter<ReqwestPost>, String> {
    let alert_cfg =
        AlertConfig::from_toml(ALERTS).map_err(|e| format!("config/alerts.toml: {e}"))?;
    let http = ReqwestPost::new(alert_cfg.telegram_timeout).map_err(|e| e.to_string())?;
    let telegram = TelegramSink::from_env(&alert_cfg, http, |n| std::env::var(n).ok());
    Ok(Alerter::new(
        alert_cfg,
        Arc::new(Logger::stderr()),
        telegram,
    ))
}

/// Pantalla alternativa y modo crudo, deshechos al salir (también si hay un pánico).
struct Tui;

impl Tui {
    fn enter() -> std::io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = std::io::stdout();
        if let Err(e) = execute!(out, terminal::EnterAlternateScreen, cursor::Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(e);
        }
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            restore();
            prev(info);
        }));
        Ok(Tui)
    }
}

fn restore() {
    let _ = execute!(
        std::io::stdout(),
        ResetColor,
        cursor::Show,
        terminal::LeaveAlternateScreen
    );
    let _ = terminal::disable_raw_mode();
}

impl Drop for Tui {
    fn drop(&mut self) {
        restore();
    }
}

fn color(t: Tone) -> Color {
    match t {
        Tone::Plain => Color::Reset,
        Tone::Up => Color::Green,
        Tone::Down => Color::Red,
        Tone::Dim => Color::DarkGrey,
        Tone::Warn => Color::Yellow,
        Tone::Accent => Color::Cyan,
        Tone::Bar => Color::White,
    }
}

fn paint(lines: &[Line], mono: bool) -> std::io::Result<()> {
    let mut out = std::io::stdout().lock();
    queue!(out, cursor::MoveTo(0, 0))?;
    for (i, l) in lines.iter().enumerate() {
        queue!(out, cursor::MoveTo(0, i as u16))?;
        for s in l {
            if mono {
                queue!(out, Print(&s.text))?;
            } else {
                let bg = if s.tone == Tone::Bar {
                    Color::DarkBlue
                } else {
                    Color::Reset
                };
                queue!(
                    out,
                    SetBackgroundColor(bg),
                    SetForegroundColor(color(s.tone)),
                    Print(&s.text)
                )?;
            }
        }
        queue!(out, ResetColor, terminal::Clear(ClearType::UntilNewLine))?;
    }
    queue!(out, terminal::Clear(ClearType::FromCursorDown))?;
    out.flush()
}

fn activity(app: &App, busy: bool, now: DateTime<Utc>, p: &Periods) -> String {
    if app.trip.is_some() {
        return "q para salir".into();
    }
    if busy {
        return "pidiendo velas…".into();
    }
    let wait = app
        .sched
        .ready_at(p)
        .map(|t| (t - now).num_seconds().max(0))
        .unwrap_or(0);
    format!(
        "próximo pedido en {wait} s (máx. {}/10 min)",
        app.cfg.max_requests_per_10_min()
    )
}

fn draw(app: &App, busy: bool, mono: bool) -> std::io::Result<()> {
    let (w, h) = terminal::size()?;
    let now = Utc::now();
    let p = app.periods(false);
    let rows = app.rows();
    let size = app.size();
    let chart = app.charts.get(&(app.selected, size));
    let (bars, error, contract, source) = match chart {
        Some(Ok(c)) => (
            Some(c.bars.as_slice()),
            None,
            Some(c.contract.as_str()),
            Some(c.source.as_str()),
        ),
        Some(Err(e)) => (None, Some(e.as_str()), None, None),
        None => (None, None, None, None),
    };
    let screen = Screen {
        now,
        rows: &rows,
        selected: app.selected,
        chart: ChartView {
            contract,
            source,
            size,
            sizes: &app.cfg.timeframes,
            bars,
            error,
        },
        link: match &app.trip {
            Some(t) => Link::KillSwitch {
                reason: t.reason.clone(),
            },
            None => Link::Connected {
                address: app.address.clone(),
            },
        },
        activity: activity(app, busy, now, &p),
        mono,
    };
    paint(&view::render(&screen, w as usize, h as usize), mono)
}

/// Pantalla completa hasta que se aprieta q. Devuelve true si se activó el kill switch.
async fn run_tui(
    app: &mut App,
    session: &Arc<Session>,
    alerts: &Alerter<ReqwestPost>,
    mono: bool,
) -> std::io::Result<bool> {
    let _tui = Tui::enter()?;
    let (tx, mut keys) = mpsc::channel::<Event>(64);
    // Las teclas se leen en un hilo aparte (la lectura bloquea); se corta al salir del programa.
    std::thread::spawn(move || {
        while let Ok(ev) = crossterm::event::read() {
            if tx.blocking_send(ev).is_err() {
                break;
            }
        }
    });
    let mut tick = tokio::time::interval(app.cfg.redraw);
    let mut inflight: Option<(Task, JoinHandle<Result<SourcedBars, SessionError>>)> = None;
    let periods = app.periods(false);
    loop {
        let now = Utc::now();
        if inflight.is_none() && app.trip.is_none() {
            let n = app.ids.len();
            if let Some(task) = app.sched.next(now, n, (app.selected, app.size()), &periods) {
                app.sched.record(task, now);
                let (i, size, from) = app.plan(task, now);
                let h = tokio::spawn(fetch(session.clone(), app.ids[i].clone(), size, from));
                inflight = Some((task, h));
            }
        }
        draw(app, inflight.is_some(), mono)?;
        tokio::select! {
            ev = keys.recv() => match ev {
                Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    let ctrl_c = k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL);
                    if ctrl_c || matches!(k.code, KeyCode::Char('q') | KeyCode::Esc) {
                        break;
                    }
                    app.key(k.code);
                }
                Some(_) => {}
                None => break,
            },
            res = async {
                match inflight.as_mut() {
                    Some((_, h)) => h.await,
                    None => std::future::pending().await,
                }
            } => {
                let task = inflight.take().map(|(t, _)| t);
                if let Some(task) = task {
                    let got = res.unwrap_or_else(|e| Err(SessionError::Rejected(cfds_ibkr::ApiError::Data(e.to_string()))));
                    if let Some(trip) = app.apply(task, got) {
                        alerts.raise(trip.kind, &trip.reason, trip.at).await;
                    }
                }
            },
            _ = tick.tick() => {}
        }
    }
    if let Some((_, h)) = inflight {
        h.abort();
    }
    Ok(app.trip.is_some())
}

/// `--once`: último precio y variación de cada instrumento, una vez, como texto.
async fn run_once(app: &mut App, session: &Arc<Session>, alerts: &Alerter<ReqwestPost>) -> bool {
    let periods = app.periods(true);
    let n = app.ids.len();
    let wanted: Vec<Task> = (0..n)
        .flat_map(|i| [Task::Quote(i), Task::Daily(i)])
        .collect();
    for task in wanted {
        if let Some(wait) = app.sched.ready_at(&periods).map(|t| t - Utc::now())
            && let Ok(d) = wait.to_std()
        {
            tokio::time::sleep(d).await;
        }
        let now = Utc::now();
        app.sched.record(task, now);
        let (i, size, from) = app.plan(task, now);
        let got = fetch(session.clone(), app.ids[i].clone(), size, from).await;
        if let Some(trip) = app.apply(task, got) {
            eprintln!("KILL SWITCH ({}): {}", trip.kind.as_str(), trip.reason);
            alerts.raise(trip.kind, &trip.reason, trip.at).await;
            return true;
        }
    }
    for l in view::render_table(Utc::now(), &app.rows()) {
        println!("{l}");
    }
    false
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!(
            "bolsa [--once | --check-config] [--no-color]  (ver docs/guides/terminal-bolsa.md)"
        );
        return ExitCode::SUCCESS;
    }
    let once = args.iter().any(|a| a == "--once");
    let cfg = match TerminalConfig::from_toml(TERMINAL) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let mut ibkr = match IbkrConfig::from_toml(IBKR) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    if let Err(e) = cfg.check_against(&ibkr) {
        eprintln!("{e}");
        return ExitCode::from(1);
    }
    ibkr.client_id = cfg.client_id;
    let ibkr_view = ibkr.clone();
    if args.iter().any(|a| a == "--check-config") {
        println!(
            "config/terminal.toml válida: {} en {}:{} (client_id {}), timeframes {}, pausa mínima {} s \
             entre pedidos (máx. {} pedidos/10 min; piso {MIN_GAP_SECS} s)",
            cfg.instruments
                .iter()
                .map(|i| i.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            ibkr.host,
            ibkr.port,
            cfg.client_id,
            cfg.timeframes
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            cfg.min_request_gap.num_seconds(),
            cfg.max_requests_per_10_min(),
        );
        return ExitCode::SUCCESS;
    }
    let tty = std::io::stdout().is_terminal();
    if !once && !tty {
        eprintln!("la salida no es una terminal: usá --once para ver la tabla una vez");
        return ExitCode::from(1);
    }
    let mono = args.iter().any(|a| a == "--no-color")
        || std::env::var("NO_COLOR").is_ok_and(|v| !v.is_empty())
        || std::env::var("TERM").is_ok_and(|v| v == "dumb");
    let alerts = match alerter() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    eprintln!(
        "conectando a IB Gateway en {}:{} (solo lectura, client_id {})...",
        ibkr.host, ibkr.port, ibkr.client_id
    );
    let session = match connect(ibkr, |k| std::env::var(k).ok()).await {
        Ok(s) => Arc::new(s),
        Err(ConnectError::Tripped(trip)) => {
            eprintln!("KILL SWITCH ({}): {}", trip.kind.as_str(), trip.reason);
            alerts.raise(trip.kind, &trip.reason, trip.at).await;
            return ExitCode::from(2);
        }
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };
    let mut app = App::new(cfg, &ibkr_view);
    let tripped = if once {
        run_once(&mut app, &session, &alerts).await
    } else {
        match run_tui(&mut app, &session, &alerts, mono).await {
            Ok(t) => t,
            Err(e) => {
                eprintln!("terminal: {e}");
                if let Ok(s) = Arc::try_unwrap(session) {
                    s.close().await;
                }
                return ExitCode::from(1);
            }
        }
    };
    if let Some(t) = &app.trip {
        eprintln!("KILL SWITCH ({}): {}", t.kind.as_str(), t.reason);
    }
    if let Ok(s) = Arc::try_unwrap(session) {
        s.close().await;
    }
    if tripped {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}
