//! Sorpresa del calendario (ADR 0018, #698): consenso sin mirar el futuro, dato real `first` a la
//! hora del evento, transformaciones y horario de verano.

use std::path::PathBuf;

use cfds_core::{CalendarEstimate, ReleaseActual, StatUnit, Vintage};
use cfds_market_data::surprise::{
    NoSurprise, SurpriseConfig, SurpriseMapping, Transform, consensus_before, surprise,
};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::{Decimal, dec};

fn t(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}

/// CPI de octubre de 2026: 10-11-2026 08:30 ET = 13:30Z (horario de invierno).
const AT: &str = "2026-11-10T13:30:00Z";

fn est(event: &str, at: &str, forecast: Option<Decimal>, seen: &str) -> CalendarEstimate {
    CalendarEstimate {
        source: "forex_factory".into(),
        event: event.into(),
        country: "US".into(),
        scheduled_at: t(at),
        forecast_raw: forecast.map(|f| format!("{f}%")),
        previous_raw: Some("0.6%".into()),
        forecast_value: forecast,
        previous_value: Some(dec!(0.6)),
        unit: Some(StatUnit::Percent),
        seen_at: t(seen),
    }
}

#[allow(clippy::too_many_arguments)]
fn act(
    sid: &str,
    period: &str,
    value: Decimal,
    unit: StatUnit,
    vintage: Vintage,
    released: &str,
    retrieved: &str,
) -> ReleaseActual {
    ReleaseActual {
        source: "bls".into(),
        series_id: sid.into(),
        reference_period: period.into(),
        value,
        unit,
        vintage,
        released_at: t(released),
        retrieved_at: t(retrieved),
        source_url: "https://api.bls.gov/publicAPI/v1/timeseries/data/".into(),
    }
}

fn mapping(event: &str, sid: &str, transform: Transform) -> SurpriseMapping {
    SurpriseMapping {
        event: event.into(),
        country: "US".into(),
        series_id: sid.into(),
        transform,
    }
}

/// Valores reales del BLS leídos el 25-11-2026 (CPI-U desestacionalizado).
fn cpi_actuals() -> Vec<ReleaseActual> {
    vec![
        act(
            "CUSR0000SA0",
            "2026-09",
            dec!(336.100),
            StatUnit::Level,
            Vintage::First,
            "2026-10-14T12:30:00Z",
            "2026-10-15T15:00:00Z",
        ),
        // Revisión del mes anterior leída junto con el primer dato de octubre: es la que vale.
        act(
            "CUSR0000SA0",
            "2026-09",
            dec!(336.298),
            StatUnit::Level,
            Vintage::Revised,
            "2026-10-14T12:30:00Z",
            "2026-11-11T15:00:00Z",
        ),
        act(
            "CUSR0000SA0",
            "2026-10",
            dec!(337.476),
            StatUnit::Level,
            Vintage::First,
            AT,
            "2026-11-11T15:00:00Z",
        ),
        // Una revisión posterior (leída después) no cambia nada.
        act(
            "CUSR0000SA0",
            "2026-09",
            dec!(999),
            StatUnit::Level,
            Vintage::Revised,
            "2026-10-14T12:30:00Z",
            "2026-12-11T15:00:00Z",
        ),
    ]
}

#[test]
fn pct_change_uses_the_previous_month_known_when_the_first_value_was_read() {
    let m = mapping("CPI m/m", "CUSR0000SA0", Transform::PctChange);
    let estimates = [est("CPI m/m", AT, Some(dec!(0.3)), "2026-11-09T10:00:00Z")];
    let s = surprise(&m, t(AT), &estimates, &cpi_actuals()).unwrap();
    let pct = (dec!(337.476) / dec!(336.298) - Decimal::ONE) * Decimal::ONE_HUNDRED;
    assert_eq!(s.actual, pct.normalize());
    assert!(s.actual > dec!(0.35) && s.actual < dec!(0.36)); // ≈ 0.3503 %, sin redondear
    assert_eq!(s.consensus, dec!(0.3));
    assert_eq!(s.surprise, s.actual - dec!(0.3));
    assert_eq!(s.unit, StatUnit::Percent);
    assert_eq!(s.reference_period, "2026-10");
    assert_eq!(
        (s.released_at, s.retrieved_at),
        (t(AT), t("2026-11-11T15:00:00Z"))
    );
}

#[test]
fn no_look_ahead_the_consensus_is_the_last_snapshot_strictly_before_the_event() {
    let at = t(AT);
    let estimates = [
        est("CPI m/m", AT, Some(dec!(0.2)), "2026-11-06T10:00:00Z"),
        est("CPI m/m", AT, Some(dec!(0.3)), "2026-11-10T13:29:59Z"),
        est("CPI m/m", AT, Some(dec!(0.9)), AT), // a la hora exacta: no cuenta
        est("CPI m/m", AT, Some(dec!(0.8)), "2026-11-10T15:00:00Z"), // después: no cuenta
        est("Core CPI m/m", AT, Some(dec!(0.7)), "2026-11-10T12:00:00Z"), // otro evento
        est(
            "CPI m/m",
            "2026-11-11T13:30:00Z",
            Some(dec!(0.6)),
            "2026-11-10T12:00:00Z",
        ), // otra hora
    ];
    let c = consensus_before(&estimates, "CPI m/m", "US", at).unwrap();
    assert_eq!(c.forecast_value, Some(dec!(0.3)));
    // Lo que se ve después de la hora no cambia la sorpresa (sin repintado).
    let m = mapping("CPI m/m", "CUSR0000SA0", Transform::PctChange);
    let before = surprise(&m, at, &estimates[..2], &cpi_actuals()).unwrap();
    let all = surprise(&m, at, &estimates, &cpi_actuals()).unwrap();
    assert_eq!(before, all);
    // Solo instantáneas en o después de la hora: no hay consenso.
    assert_eq!(
        surprise(&m, at, &estimates[2..4], &cpi_actuals()),
        Err(NoSurprise::NoConsensus)
    );
}

#[test]
fn change_and_level_transforms() {
    // Empleo de octubre: viernes 6-11-2026 08:30 ET = 13:30Z.
    let at = "2026-11-06T13:30:00Z";
    let read = "2026-11-09T15:00:00Z";
    let actuals = [
        act(
            "CES0000000001",
            "2026-09",
            dec!(159128),
            StatUnit::Thousands,
            Vintage::Revised,
            "2026-10-02T12:30:00Z",
            read,
        ),
        act(
            "CES0000000001",
            "2026-10",
            dec!(159293),
            StatUnit::Thousands,
            Vintage::First,
            at,
            read,
        ),
        act(
            "LNS14000000",
            "2026-10",
            dec!(4.1),
            StatUnit::Percent,
            Vintage::First,
            at,
            read,
        ),
    ];
    let mut nfp = est(
        "Non-Farm Employment Change",
        at,
        Some(dec!(150)),
        "2026-11-05T10:00:00Z",
    );
    nfp.unit = Some(StatUnit::Thousands);
    let s = surprise(
        &mapping(
            "Non-Farm Employment Change",
            "CES0000000001",
            Transform::Change,
        ),
        t(at),
        &[nfp],
        &actuals,
    )
    .unwrap();
    assert_eq!((s.actual, s.surprise), (dec!(165), dec!(15)));
    let ur = est(
        "Unemployment Rate",
        at,
        Some(dec!(4.2)),
        "2026-11-05T10:00:00Z",
    );
    let s = surprise(
        &mapping("Unemployment Rate", "LNS14000000", Transform::Level),
        t(at),
        &[ur],
        &actuals,
    )
    .unwrap();
    assert_eq!((s.actual, s.surprise), (dec!(4.1), dec!(-0.1)));
}

#[test]
fn dst_the_actual_must_be_released_at_the_same_utc_hour_as_the_event() {
    // 11-3-2026 08:30 EDT = 12:30Z (ya en horario de verano desde el 8-3).
    let at = "2026-03-11T12:30:00Z";
    let read = "2026-03-12T15:00:00Z";
    let actuals = [act(
        "LNS14000000",
        "2026-02",
        dec!(4.3),
        StatUnit::Percent,
        Vintage::First,
        at,
        read,
    )];
    let m = mapping("Unemployment Rate", "LNS14000000", Transform::Level);
    let e = est(
        "Unemployment Rate",
        at,
        Some(dec!(4.2)),
        "2026-03-11T12:29:00Z",
    );
    assert_eq!(
        surprise(&m, t(at), std::slice::from_ref(&e), &actuals)
            .unwrap()
            .surprise,
        dec!(0.1)
    );
    // Un evento cargado una hora corrida (13:30Z, la hora de invierno) no se empareja.
    let wrong = t(at) + Duration::hours(1);
    let mut e2 = e;
    e2.scheduled_at = wrong;
    assert_eq!(
        surprise(&m, wrong, &[e2], &actuals),
        Err(NoSurprise::NoActual)
    );
}

#[test]
fn edge_cases_never_invent_a_surprise() {
    let at = t(AT);
    let m = mapping("CPI m/m", "CUSR0000SA0", Transform::PctChange);
    let good = est("CPI m/m", AT, Some(dec!(0.3)), "2026-11-09T10:00:00Z");
    // Forecast sin número (por ejemplo "<0.1%").
    let mut no_value = good.clone();
    no_value.forecast_value = None;
    no_value.previous_value = None;
    no_value.unit = None;
    assert_eq!(
        surprise(&m, at, &[no_value], &cpi_actuals()),
        Err(NoSurprise::NoForecastValue)
    );
    // Solo valores revisados: no hay primer dato.
    let revised: Vec<_> = cpi_actuals()
        .into_iter()
        .map(|mut a| {
            a.vintage = Vintage::Revised;
            a
        })
        .collect();
    assert_eq!(
        surprise(&m, at, std::slice::from_ref(&good), &revised),
        Err(NoSurprise::NoActual)
    );
    // Sin el mes anterior conocido al leer el primer dato.
    let only_oct: Vec<_> = cpi_actuals()
        .into_iter()
        .filter(|a| a.reference_period == "2026-10")
        .collect();
    assert_eq!(
        surprise(&m, at, std::slice::from_ref(&good), &only_oct),
        Err(NoSurprise::NoPrevious)
    );
    // Mes anterior en cero.
    let mut zero = cpi_actuals();
    zero.retain(|a| a.reference_period == "2026-10");
    zero.push(act(
        "CUSR0000SA0",
        "2026-09",
        dec!(0),
        StatUnit::Level,
        Vintage::First,
        "2026-10-14T12:30:00Z",
        "2026-10-15T15:00:00Z",
    ));
    assert_eq!(
        surprise(&m, at, std::slice::from_ref(&good), &zero),
        Err(NoSurprise::ZeroPrevious)
    );
    // Unidad distinta (el forecast en miles contra una variación %).
    let mut thousands = good.clone();
    thousands.unit = Some(StatUnit::Thousands);
    assert_eq!(
        surprise(&m, at, &[thousands], &cpi_actuals()),
        Err(NoSurprise::UnitMismatch {
            forecast: StatUnit::Thousands,
            actual: StatUnit::Percent
        })
    );
    // Dos primeros datos a la misma hora.
    let mut two = cpi_actuals();
    two.push(act(
        "CUSR0000SA0",
        "2026-11",
        dec!(338),
        StatUnit::Level,
        Vintage::First,
        AT,
        "2026-11-11T15:00:00Z",
    ));
    assert_eq!(
        surprise(&m, at, &[good], &two),
        Err(NoSurprise::AmbiguousActual)
    );
}

#[test]
fn january_compares_against_december_of_the_previous_year() {
    let at = "2027-01-13T13:30:00Z";
    let read = "2027-01-14T15:00:00Z";
    let actuals = [
        act(
            "CUSR0000SA0",
            "2026-12",
            dec!(200),
            StatUnit::Level,
            Vintage::Revised,
            "2026-12-10T13:30:00Z",
            read,
        ),
        act(
            "CUSR0000SA0",
            "2027-01",
            dec!(201),
            StatUnit::Level,
            Vintage::First,
            at,
            read,
        ),
    ];
    let m = mapping("CPI m/m", "CUSR0000SA0", Transform::PctChange);
    let e = est("CPI m/m", at, Some(dec!(0.4)), "2027-01-12T10:00:00Z");
    let s = surprise(&m, t(at), &[e], &actuals).unwrap();
    assert_eq!((s.actual, s.surprise), (dec!(0.5), dec!(0.1)));
}

#[test]
fn the_repository_config_maps_the_bls_series() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../config/calendar_surprise.toml");
    let cfg = SurpriseConfig::load(&path).unwrap();
    let m = cfg.mapping("CPI m/m", "US").unwrap();
    assert_eq!(
        (m.series_id.as_str(), m.transform),
        ("CUSR0000SA0", Transform::PctChange)
    );
    assert_eq!(
        cfg.mapping("Non-Farm Employment Change", "US")
            .unwrap()
            .transform,
        Transform::Change
    );
    assert!(cfg.mapping("CPI m/m", "EU").is_none());
    let bad = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\"pct_change\"", "\"ratio\"");
    assert!(SurpriseConfig::from_toml(&bad).is_err());
}
