//! Qué pedido de velas toca ahora. Un solo pedido a la vez y nunca dos más cerca que la pausa mínima
//! de la configuración: así el programa entero queda por debajo de 60 pedidos en 10 minutos, sin
//! importar cuántos instrumentos haya ni cuántas veces se cambie de gráfico.

use std::collections::HashMap;

use cfds_ibkr::BarSize;
use chrono::{DateTime, TimeDelta, Utc};

/// Un pedido de velas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Task {
    /// Velas de 1 minuto del instrumento `i` (último precio).
    Quote(usize),
    /// Velas diarias del instrumento `i` (referencia de la variación del día).
    Daily(usize),
    /// Gráfico del instrumento `i` en un timeframe.
    Chart(usize, BarSize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Periods {
    pub gap: TimeDelta,
    pub quote: TimeDelta,
    pub daily: TimeDelta,
    pub chart: TimeDelta,
}

#[derive(Debug, Clone, Default)]
pub struct Scheduler {
    last_request: Option<DateTime<Utc>>,
    done: HashMap<Task, DateTime<Utc>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Cuándo se puede hacer el próximo pedido.
    pub fn ready_at(&self, p: &Periods) -> Option<DateTime<Utc>> {
        self.last_request.map(|t| t + p.gap)
    }

    /// El pedido que toca en `now`, o ninguno (pausa mínima sin cumplir, o nada vencido).
    ///
    /// Orden: primero el gráfico que se está mirando si nunca se pidió; después lo que nunca se
    /// pidió (último precio antes que diarias, en el orden de la tabla); después lo más vencido.
    pub fn next(
        &self,
        now: DateTime<Utc>,
        instruments: usize,
        selected: (usize, BarSize),
        p: &Periods,
    ) -> Option<Task> {
        if self.ready_at(p).is_some_and(|t| now < t) {
            return None;
        }
        let chart = Task::Chart(selected.0, selected.1);
        let mut tasks = vec![chart];
        tasks.extend((0..instruments).map(Task::Quote));
        tasks.extend((0..instruments).map(Task::Daily));
        if let Some(t) = tasks.iter().find(|t| !self.done.contains_key(t)) {
            return Some(*t);
        }
        tasks
            .into_iter()
            .filter_map(|t| {
                let period = match t {
                    Task::Quote(_) => p.quote,
                    Task::Daily(_) => p.daily,
                    Task::Chart(..) => p.chart,
                };
                let due = self.done.get(&t).map(|d| *d + period)?;
                (due <= now).then_some((now - due, t))
            })
            // El más vencido; si empatan, el primero de la lista.
            .fold(None, |best: Option<(TimeDelta, Task)>, c| match best {
                Some(b) if b.0 >= c.0 => Some(b),
                _ => Some(c),
            })
            .map(|(_, t)| t)
    }

    /// Anota un pedido hecho (haya salido bien o no: un error se reintenta en el período siguiente,
    /// no enseguida).
    pub fn record(&mut self, task: Task, at: DateTime<Utc>) {
        self.last_request = Some(at);
        self.done.insert(task, at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn p() -> Periods {
        Periods {
            gap: TimeDelta::seconds(15),
            quote: TimeDelta::seconds(60),
            daily: TimeDelta::seconds(1800),
            chart: TimeDelta::seconds(60),
        }
    }

    fn t0() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 11, 25, 14, 0, 0).unwrap()
    }

    fn s(n: i64) -> TimeDelta {
        TimeDelta::seconds(n)
    }

    #[test]
    fn first_round_chart_then_quotes_then_dailies() {
        let mut sc = Scheduler::new();
        let sel = (1, BarSize::M15);
        let mut got = Vec::new();
        let mut now = t0();
        for _ in 0..5 {
            let t = sc.next(now, 2, sel, &p()).unwrap();
            sc.record(t, now);
            got.push(t);
            now += s(15);
        }
        assert_eq!(
            got,
            vec![
                Task::Chart(1, BarSize::M15),
                Task::Quote(0),
                Task::Quote(1),
                Task::Daily(0),
                Task::Daily(1),
            ]
        );
    }

    #[test]
    fn respects_the_minimum_gap() {
        let mut sc = Scheduler::new();
        let sel = (0, BarSize::M1);
        sc.record(Task::Chart(0, BarSize::M1), t0());
        assert_eq!(sc.next(t0() + s(14), 1, sel, &p()), None);
        assert!(sc.next(t0() + s(15), 1, sel, &p()).is_some());
        assert_eq!(sc.ready_at(&p()), Some(t0() + s(15)));
    }

    #[test]
    fn new_chart_selection_jumps_the_queue() {
        let mut sc = Scheduler::new();
        let mut now = t0();
        for t in [Task::Chart(0, BarSize::M15), Task::Quote(0), Task::Daily(0)] {
            sc.record(t, now);
            now += s(15);
        }
        assert_eq!(
            sc.next(now, 1, (0, BarSize::H1), &p()),
            Some(Task::Chart(0, BarSize::H1))
        );
    }

    #[test]
    fn nothing_due_then_most_overdue_first() {
        let mut sc = Scheduler::new();
        let sel = (0, BarSize::M15);
        sc.record(Task::Chart(0, BarSize::M15), t0());
        sc.record(Task::Quote(0), t0() + s(15));
        sc.record(Task::Daily(0), t0() + s(30));
        assert_eq!(sc.next(t0() + s(50), 1, sel, &p()), None);
        // A los 80 s vencieron el gráfico (hace 20 s) y el precio (hace 5 s): primero el gráfico.
        assert_eq!(
            sc.next(t0() + s(80), 1, sel, &p()),
            Some(Task::Chart(0, BarSize::M15))
        );
    }

    #[test]
    fn rate_never_exceeds_gap_over_ten_minutes() {
        let mut sc = Scheduler::new();
        let mut now = t0();
        let end = t0() + TimeDelta::minutes(10);
        let mut count = 0;
        let sizes = [BarSize::M1, BarSize::M15, BarSize::H1, BarSize::D1];
        let mut k = 0;
        while now < end {
            // Cambia de gráfico seguido, como alguien apretando teclas.
            k += 1;
            let sel = (k % 3, sizes[k % 4]);
            if let Some(t) = sc.next(now, 3, sel, &p()) {
                sc.record(t, now);
                count += 1;
            }
            now += s(1);
        }
        assert!(count <= 40, "{count}");
    }
}
