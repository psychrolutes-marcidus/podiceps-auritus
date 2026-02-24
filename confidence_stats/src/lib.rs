use chrono::Utc;
use duckdb::Result;
use rust_decimal::Decimal;
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

fn get_draught(conn: &duckdb::Connection, mmsi: u32) -> Vec<u64> {
    let result: Vec<(chrono::DateTime<Utc>, Option<Decimal>)> = conn
        .prepare("SELECT timestamp, draught FROM data WHERE mmsi = ? ORDER BY timestamp")
        .unwrap()
        .query_map([mmsi], |row| Ok((row.get(0).unwrap(), row.get(1).unwrap())))
        .unwrap()
        .collect::<Result<_>>()
        .unwrap();
    todo!()
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
        mmsis.iter().for_each(|x| {
            get_draught(&conn, *x);
        });

        assert!(false)
    }
}
