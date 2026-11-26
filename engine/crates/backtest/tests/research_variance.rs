//! Simulador de varianza (#801, `research::variance`): determinismo, casos extremos que se
//! calculan a mano, ruina, rachas, la Beta de la incertidumbre del win rate y el remuestreo de R
//! reales.
#![allow(clippy::float_arithmetic, reason = "estadísticos de caminos simulados")]

use cfds_backtest::research::program4::SplitMix64;
use cfds_backtest::research::variance::{beta, simulate};
use cfds_backtest::research::{Payoff, RSource, VarianceParams};

fn params(risk: f64, trades: usize, paths: usize) -> VarianceParams {
    VarianceParams {
        risk_fraction: risk,
        trades,
        paths,
        seed: 7,
        ruin_threshold: 0.30,
        streak_max_k: 10,
        drawdown_bin: 0.05,
    }
}

fn model(win_rate: f64, win: f64, loss: f64, observed: Option<u32>) -> RSource<'static> {
    RSource::Model(Payoff {
        win_rate,
        avg_win_r: win,
        avg_loss_r: loss,
        observed_trades: observed,
    })
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn same_seed_same_report_and_another_seed_changes_it() {
    let src = model(0.5, 1.5, 1.0, Some(40));
    let p = params(0.02, 200, 500);
    let a = simulate(&src, &p).unwrap();
    assert_eq!(a, simulate(&src, &p).unwrap());
    let b = simulate(&src, &VarianceParams { seed: 8, ..p }).unwrap();
    assert_ne!(a.bands, b.bands);
}

#[test]
fn always_winning_grows_the_same_on_every_path() {
    let rep = simulate(&model(1.0, 2.0, 1.0, None), &params(0.01, 50, 100)).unwrap();
    let expected = 1.02_f64.powi(50);
    let last = rep.bands.last().unwrap();
    assert_eq!(last.trade, 50);
    for q in [last.p5, last.p25, last.p50, last.p75, last.p95] {
        assert!(close(q, expected), "{q} != {expected}");
    }
    assert_eq!(rep.ruined, 0);
    assert!(close(rep.max_drawdown.p95, 0.0));
    assert!(rep.streak_at_least.iter().all(|(_, p)| *p == 0.0));
    assert!(close(rep.expectancy_r, 2.0));
    assert!(rep.win_rate_draws.is_none());
}

#[test]
fn always_losing_ruins_with_enough_risk_and_the_streak_is_every_trade() {
    let rep = simulate(&model(0.0, 2.0, 1.0, None), &params(0.05, 20, 50)).unwrap();
    let expected = 0.95_f64.powi(20); // 0,358: tocó 0,70
    assert!(close(rep.bands.last().unwrap().p50, expected));
    assert_eq!(rep.ruined, 50);
    assert!(close(rep.risk_of_ruin(), 1.0));
    assert!(close(rep.longest_losing_streak.p5, 20.0));
    assert!(rep.streak_at_least.iter().all(|(_, p)| *p == 1.0));
    assert!(close(rep.max_drawdown.p50, 1.0 - expected));
}

#[test]
fn more_risk_means_more_ruin_with_the_same_edge() {
    let src = model(0.45, 1.5, 1.0, None);
    let low = simulate(&src, &params(0.01, 300, 2000)).unwrap();
    let high = simulate(&src, &params(0.05, 300, 2000)).unwrap();
    assert!(high.risk_of_ruin() > low.risk_of_ruin());
    assert!(high.max_drawdown.p50 > low.max_drawdown.p50);
}

#[test]
fn bands_start_at_one_end_at_the_last_trade_and_are_ordered() {
    let rep = simulate(&model(0.5, 1.2, 1.0, None), &params(0.01, 1000, 200)).unwrap();
    let first = rep.bands.first().unwrap();
    assert_eq!(first.trade, 0);
    assert!(close(first.p5, 1.0) && close(first.p95, 1.0));
    assert_eq!(rep.bands.last().unwrap().trade, 1000);
    assert!(rep.bands.len() <= 102);
    for b in &rep.bands {
        assert!(b.p5 <= b.p25 && b.p25 <= b.p50 && b.p50 <= b.p75 && b.p75 <= b.p95);
    }
    assert!(rep.bands.windows(2).all(|w| w[0].trade < w[1].trade));
}

#[test]
fn histogram_covers_every_path_and_streak_odds_only_go_down() {
    let rep = simulate(&model(0.4, 1.5, 1.0, None), &params(0.02, 200, 1000)).unwrap();
    let total: f64 = rep.drawdown_histogram.iter().map(|b| b.share).sum();
    assert!(close(total, 1.0), "{total}");
    assert_eq!(rep.drawdown_histogram.len(), 20);
    let odds: Vec<f64> = rep.streak_at_least.iter().map(|(_, p)| *p).collect();
    assert!(odds.windows(2).all(|w| w[0] >= w[1]));
    assert_eq!(rep.streak_at_least.first().unwrap().0, 1);
    assert_eq!(rep.streak_at_least.len(), 10);
    // Con 200 operaciones al 40 % es casi seguro perder al menos una.
    assert!(odds[0] > 0.999);
}

#[test]
fn beta_draws_center_on_the_posterior_mean() {
    let mut rng = SplitMix64::new(3);
    let (a, b) = (0.55 * 40.0 + 1.0, 0.45 * 40.0 + 1.0);
    let draws: Vec<f64> = (0..20_000).map(|_| beta(&mut rng, a, b)).collect();
    let mean = draws.iter().sum::<f64>() / draws.len() as f64;
    assert!((mean - a / (a + b)).abs() < 0.005, "{mean}");
    assert!(draws.iter().all(|x| (0.0..=1.0).contains(x)));
    let var = draws.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / draws.len() as f64;
    let expected = a * b / ((a + b).powi(2) * (a + b + 1.0));
    assert!((var - expected).abs() < 0.001, "{var} vs {expected}");
}

#[test]
fn uncertainty_widens_the_outcomes_and_reports_the_drawn_win_rates() {
    let p = params(0.02, 300, 3000);
    let fixed = simulate(&model(0.55, 1.0, 1.0, None), &p).unwrap();
    let unsure = simulate(&model(0.55, 1.0, 1.0, Some(40)), &p).unwrap();
    let (f, u) = (fixed.bands.last().unwrap(), unsure.bands.last().unwrap());
    assert!(u.p5 < f.p5 && u.p95 > f.p95);
    assert!(unsure.risk_of_ruin() >= fixed.risk_of_ruin());
    let draws = unsure.win_rate_draws.unwrap();
    assert!(draws.p5 < 0.5 && draws.p95 > 0.6);
    // Con muchas operaciones observadas la Beta se cierra sobre el win rate medido.
    let tight = simulate(&model(0.55, 1.0, 1.0, Some(100_000)), &p).unwrap();
    let t = tight.win_rate_draws.unwrap();
    assert!(t.p95 - t.p5 < 0.01);
}

#[test]
fn resampled_r_keep_their_mean_and_an_all_positive_list_never_ruins() {
    let r = [1.0, -1.0, 2.0, -1.0];
    let rep = simulate(
        &RSource::Resample {
            r: &r,
            block_len: 1,
        },
        &params(0.01, 30, 100),
    )
    .unwrap();
    assert!(close(rep.expectancy_r, 0.25));
    let pos = [0.5, 1.0];
    let rep = simulate(
        &RSource::Resample {
            r: &pos,
            block_len: 2,
        },
        &params(0.10, 500, 100),
    )
    .unwrap();
    assert_eq!(rep.ruined, 0);
    assert!(close(rep.max_drawdown.p95, 0.0));
}

#[test]
fn invalid_inputs_do_not_simulate() {
    let ok = params(0.01, 10, 10);
    assert!(simulate(&model(1.1, 1.0, 1.0, None), &ok).is_none());
    assert!(simulate(&model(0.5, 0.0, 1.0, None), &ok).is_none());
    assert!(simulate(&model(0.5, 1.0, -1.0, None), &ok).is_none());
    assert!(simulate(&model(0.5, f64::NAN, 1.0, None), &ok).is_none());
    let src = model(0.5, 1.0, 1.0, None);
    assert!(simulate(&src, &params(0.0, 10, 10)).is_none());
    assert!(simulate(&src, &params(0.01, 0, 10)).is_none());
    assert!(simulate(&src, &params(0.01, 10, 0)).is_none());
    assert!(
        simulate(
            &src,
            &VarianceParams {
                ruin_threshold: 0.0,
                ..ok
            }
        )
        .is_none()
    );
    assert!(
        simulate(
            &RSource::Resample {
                r: &[],
                block_len: 1
            },
            &ok
        )
        .is_none()
    );
    assert!(
        simulate(
            &RSource::Resample {
                r: &[1.0, f64::INFINITY],
                block_len: 1
            },
            &ok
        )
        .is_none()
    );
}
