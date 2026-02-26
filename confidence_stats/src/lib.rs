use chrono::{DateTime, Timelike, Utc};
use duckdb::Result;
use rust_decimal::prelude::ToPrimitive;

#[derive(Debug)]
struct DraughtSegments(u64, f32);

type DraughtRow = (DateTime<Utc>, DateTime<Utc>, f32);

fn get_unique_vessels(conn: &duckdb::Connection) -> Vec<u32> {
    let mmsis: Vec<u32> = conn
        .prepare("SELECT DISTINCT mmsi FROM data")
        .unwrap()
        .query_map([], |row| Ok(row.get(0).unwrap()))
        .unwrap()
        .collect::<Result<_>>()
        .unwrap();
    mmsis
}

fn get_draught_combine(
    data: &[(chrono::DateTime<Utc>, Option<rust_decimal::Decimal>)],
) -> Vec<DraughtRow> {
    let new_result: Vec<_> = data
        .chunk_by(|a, b| a.1 == b.1)
        .filter_map(|d| match d.first().unwrap().1 {
            Some(_) => Some((
                d.first().unwrap().0,
                d.last().unwrap().0,
                d.first().unwrap().1.unwrap().to_f32().unwrap(),
            )),
            None => None,
        })
        .collect();
    new_result
        .windows(2)
        .map(|x| fix_draught_segment_time_range(x))
        .chain(new_result.last().into_iter().copied())
        .collect()
}

fn into_draught_segments(input: (DateTime<Utc>, DateTime<Utc>, f32)) -> DraughtSegments {
    DraughtSegments((input.1 - input.0).num_seconds() as u64 + 1, input.2)
}

fn fix_draught_segment_time_range(
    data: &[(DateTime<Utc>, DateTime<Utc>, f32)],
) -> (DateTime<Utc>, DateTime<Utc>, f32) {
    let start = data.first().unwrap();
    let last = data.last().unwrap();
    (start.0, last.1, start.2)
}

fn get_draught(conn: &duckdb::Connection, mmsi: u32) -> Vec<DraughtSegments> {
    let result: Vec<(chrono::DateTime<Utc>, Option<duckdb::types::Value>)> = conn
        .prepare("SELECT timestamp, draught FROM data WHERE mmsi = ? ORDER BY timestamp")
        .unwrap()
        .query_map([mmsi], |row| Ok((row.get(0).unwrap(), row.get(1).unwrap())))
        .unwrap()
        .collect::<Result<_>>()
        .unwrap();
    let data: Vec<_> = result
        .into_iter()
        .map(|(t, d)| {
            (
                t,
                d.map(|x| match x {
                    duckdb::types::Value::Decimal(decimal) => decimal,
                    _ => todo!(),
                }),
            )
        })
        .collect();

    new_result
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn setup_test_database() -> duckdb::Connection {
        let conn = duckdb::Connection::open_in_memory().unwrap();

        conn.execute(
            "CREATE TABLE data AS SELECT * FROM read_parquet(?)",
            [dbg!(
                Path::new("./aisdk-2025-01-01.pq")
                    .canonicalize()
                    .unwrap()
                    .to_str()
            )
            .unwrap()],
        )
        .unwrap();

        conn
    }

    #[test]
    fn test_name() {
        let conn = setup_test_database();

        let mmsis = get_unique_vessels(&conn);
        let mut draughts: Vec<_> = mmsis
            .iter()
            .filter_map(|x| {
                let draught = get_draught(&conn, *x);
                match draught.len() {
                    0 => None,
                    _ => Some(draught),
                }
            })
            .flatten()
            .collect();

        draughts.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        draughts.reverse();
        dbg!(&draughts);
        assert!(false)
    }
}
