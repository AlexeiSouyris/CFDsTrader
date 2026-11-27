//! Laboratorio (#775): `[worker]` de `config/lab.toml` y el avance que muestra cada etapa.

use std::time::Duration;

use cfds_backtest::lab::{JobStage, LabConfig, progress};

const LAB: &str = include_str!("../../../../config/lab.toml");

#[test]
fn the_repo_config_has_a_valid_worker_section() {
    let w = LabConfig::from_toml(LAB).unwrap().worker;
    assert_eq!(w.poll(), Duration::from_secs(w.poll_secs));
    assert_eq!(w.max_duration(), Duration::from_secs(w.max_minutes * 60));
    assert_eq!(
        w.stream_ttl(),
        Duration::from_secs(w.stream_ttl_hours * 3600)
    );
    assert!(
        w.load_from().unwrap()
            < "2015-01-01T00:00:00Z"
                .parse::<chrono::DateTime<chrono::Utc>>()
                .unwrap()
    );
    assert_eq!(w.study_args()[0], "--escenario-financiacion");
}

#[test]
fn a_bad_worker_section_fails_at_startup() {
    for (from, to) in [
        ("poll_secs = 5", "poll_secs = 0"),
        ("max_minutes = 60", "max_minutes = 0"),
        ("stream_maxlen = 1000", "stream_maxlen = 0"),
        (
            "load_from = \"2011-01-01T00:00:00Z\"",
            "load_from = \"2011-01-01T00:00:00\"",
        ),
        (
            "load_from = \"2011-01-01T00:00:00Z\"",
            "load_from = \"ayer\"",
        ),
        ("funding_scenario = \"ESTR\"", "funding_scenario = \" \""),
        ("curve_max_points = 200", "curve_max_points = 1"),
        ("curve_max_points = 200", "curve_max_points = 201"),
        ("stream_maxlen = 1000", "stream_maxlen = 1000\nthreads = 4"),
    ] {
        assert!(LAB.contains(from), "la config real cambió: {from}");
        let text = LAB.replace(from, to);
        assert!(LabConfig::from_toml(&text).is_err(), "debería fallar: {to}");
    }
    let without: String = LAB.split("[worker]").next().unwrap().to_owned();
    assert!(LabConfig::from_toml(&without).is_err());
}

#[test]
fn progress_only_goes_up_and_never_says_done() {
    use JobStage::*;
    let total = 4;
    let mut seen = vec![
        progress(Started, 0, total),
        progress(LoadingCandles, 0, total),
    ];
    // un paso antes de la primera variante y uno después de cada una (con su curva, #854)
    seen.extend((0..=total).map(|k| progress(RunningVariants, k, total)));
    seen.extend((0..total).map(|k| progress(BuildingReports, k, total)));
    seen.push(progress(Saving, total, total));
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "{seen:?}");
    assert!(seen.iter().all(|p| *p < 100));
    // Sin variantes contadas o con más de las que hay, no se rompe.
    assert_eq!(
        progress(RunningVariants, 9, 0),
        progress(RunningVariants, 1, 1)
    );
}
