use chrono::{DateTime, Timelike, Utc};
use duckdb::Result;
use rust_decimal::prelude::ToPrimitive;

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

fn fix_draught_segment_time_range(
    data: &[(DateTime<Utc>, DateTime<Utc>, f32)],
) -> (DateTime<Utc>, DateTime<Utc>, f32) {
    let start = data.first().unwrap();
    let last = data.last().unwrap();
    (start.0, last.1, start.2)
}

fn combine_same_draught(data: &[(i64, f32)]) -> (i64, f32) {
    (data.iter().map(|x| x.0).sum(), data.first().unwrap().1)
}

fn get_draught(
    conn: &duckdb::Connection,
    mmsi: u32,
) -> Vec<(chrono::DateTime<Utc>, Option<rust_decimal::Decimal>)> {
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

    data
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    const PARQUET_FILES: [&str; 67] = [
        "./aisdk-2025-01-01.pq",
        "./aisdk-2025-01-02.pq",
        "./aisdk-2025-01-03.pq",
        "./aisdk-2025-01-04.pq",
        "./aisdk-2025-01-05.pq",
        "./aisdk-2025-01-06.pq",
        "./aisdk-2025-01-07.pq",
        "./aisdk-2025-01-08.pq",
        "./aisdk-2025-01-09.pq",
        "./aisdk-2025-01-10.pq",
        "./aisdk-2025-01-11.pq",
        "./aisdk-2025-01-12.pq",
        "./aisdk-2025-01-17.pq",
        "./aisdk-2025-01-18.pq",
        "./aisdk-2025-01-19.pq",
        "./aisdk-2025-01-20.pq",
        "./aisdk-2025-01-21.pq",
        "./aisdk-2025-01-22.pq",
        "./aisdk-2025-01-23.pq",
        "./aisdk-2025-01-24.pq",
        "./aisdk-2025-01-25.pq",
        "./aisdk-2025-01-26.pq",
        "./aisdk-2025-01-27.pq",
        "./aisdk-2025-01-28.pq",
        "./aisdk-2025-01-29.pq",
        "./aisdk-2025-01-30.pq",
        "./aisdk-2025-01-31.pq",
        "./aisdk-2025-12-01.pq",
        "./aisdk-2025-12-02.pq",
        "./aisdk-2025-12-03.pq",
        "./aisdk-2025-12-04.pq",
        "./aisdk-2025-12-05.pq",
        "./aisdk-2025-12-06.pq",
        "./aisdk-2025-12-07.pq",
        "./aisdk-2025-12-08.pq",
        "./aisdk-2025-12-09.pq",
        "./aisdk-2025-12-10.pq",
        "./aisdk-2025-12-11.pq",
        "./aisdk-2025-12-12.pq",
        "./aisdk-2025-12-13.pq",
        "./aisdk-2025-12-14.pq",
        "./aisdk-2025-12-15.pq",
        "./aisdk-2025-12-16.pq",
        "./aisdk-2025-12-17.pq",
        "./aisdk-2025-12-18.pq",
        "./aisdk-2025-12-19.pq",
        "./aisdk-2025-12-20.pq",
        "./aisdk-2025-12-21.pq",
        "./aisdk-2025-12-22.pq",
        "./aisdk-2025-12-23.pq",
        "./aisdk-2025-12-24.pq",
        "./aisdk-2025-12-25.pq",
        "./aisdk-2025-12-26.pq",
        "./aisdk-2025-12-27.pq",
        "./aisdk-2025-12-28.pq",
        "./aisdk-2025-12-29.pq",
        "./aisdk-2025-12-30.pq",
        "./aisdk-2025-12-31.pq",
        "./aisdk-2026-01-01.pq",
        "./aisdk-2026-01-02.pq",
        "./aisdk-2026-01-03.pq",
        "./aisdk-2026-01-04.pq",
        "./aisdk-2026-01-05.pq",
        "./aisdk-2026-01-06.pq",
        "./aisdk-2026-01-07.pq",
        "./aisdk-2026-01-08.pq",
        "./aisdk-2026-01-09.pq",
    ];

    fn setup_test_database() -> duckdb::Connection {
        let conn = duckdb::Connection::open_in_memory().unwrap();
        conn.execute("SET memory_limit = '60GB'", []).unwrap();
        let mut par = PARQUET_FILES.iter();

        let first = par.next().unwrap();

        conn.execute(
            "CREATE TABLE data AS SELECT mmsi, timestamp, draught FROM read_parquet(?)",
            [dbg!(Path::new(*first).canonicalize().unwrap().to_str()).unwrap()],
        )
        .unwrap();

        par.for_each(|x| {
            conn.execute(
                "INSERT INTO data SELECT mmsi, timestamp, draught FROM read_parquet(?)",
                [dbg!(Path::new(x).canonicalize().unwrap().to_str()).unwrap()],
            )
            .unwrap();
        });

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
                let data = get_draught_combine(&draught);
                let mut data: Vec<_> = data
                    .iter()
                    .map(|x| ((x.1 - x.0).num_seconds(), x.2))
                    .collect();
                data.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
                let data: Vec<_> = data
                    .chunk_by(|a, b| a.1 == b.1)
                    .map(|x| combine_same_draught(x))
                    .collect();
                match data.len() {
                    0 => None,
                    _ => Some((*x, data)),
                }
            })
            .collect();

        draughts.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut new_draught: Vec<_> = draughts.iter().map(|x| x.1.clone()).flatten().collect();
        new_draught.sort_unstable_by(|a, b| a.1.total_cmp(&b.1));
        let data: Vec<_> = new_draught
            .chunk_by(|a, b| a.1 == b.1)
            .map(|x| combine_same_draught(x))
            .collect();
        // let result = draughts
        //     .iter()
        //     .fold((0_u32, vec![]), |acc: (u32, Vec<(i64, f32)>), x| {
        //         if acc.1.len() <= x.1.len() {
        //             (x.0, x.1.clone())
        //         } else {
        //             acc
        //         }
        //     });
        data.iter().for_each(|(x, y)| println!("{},{}", y, x));
        assert!(false)
    }
}
