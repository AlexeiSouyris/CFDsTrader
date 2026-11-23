//! El tercio de volatilidad (spec §13) da lo mismo que la implementación de Python de los
//! análogos que reemplazó (#158): fixture con sus valores, incluidos el calentamiento, la
//! historia corta, el 29 de febrero y la publicación exacta.

use std::path::PathBuf;

use cfds_indicators::vol_terciles;
use chrono::{DateTime, Datelike, NaiveDate, Utc, Weekday};
use rust_decimal::Decimal;
use serde_json::Value;

#[test]
fn the_terciles_match_the_reference_fixture() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vol_tercile.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let hour = u32::try_from(doc["published_hour_utc"].as_u64().unwrap()).unwrap();
    let mut day: NaiveDate = doc["start"].as_str().unwrap().parse().unwrap();
    let mut series: Vec<(NaiveDate, Option<Decimal>)> = Vec::new();
    for v in doc["realized_vol"].as_array().unwrap() {
        while matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            day = day.succ_opt().unwrap();
        }
        series.push((day, v.as_str().map(|s| s.parse().unwrap())));
        day = day.succ_opt().unwrap();
    }
    let years = u32::try_from(doc["years"].as_u64().unwrap()).unwrap();
    let terciles = vol_terciles(&series, years);
    let queries = doc["queries"].as_array().unwrap();
    assert!(queries.len() > 30);
    for q in queries {
        let as_of: DateTime<Utc> = q["as_of"].as_str().unwrap().parse().unwrap();
        let published = |d: NaiveDate| d.and_hms_opt(hour, 0, 0).unwrap().and_utc();
        let got = series
            .iter()
            .rposition(|(d, _)| published(*d) <= as_of)
            .and_then(|i| terciles[i]);
        assert_eq!(serde_json::to_value(got).unwrap(), q["expected"], "{q}");
    }
}
