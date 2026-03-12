use std::path::Path;

use crate::DatabaseError;
use duckdb::Connection;

pub fn update_db(conn: &Connection, file: &Path) -> Result<(), DatabaseError> {
    if !file.is_file() {
        return Err(DatabaseError::FileDoesNotExist);
    }

    let path = file.canonicalize()?;
    let path_str = path.to_string_lossy();

    let count: i32 = conn.query_row(
        "SELECT count(*) FROM file_store WHERE path == ?",
        [path_str.clone()],
        |row| row.get(0),
    )?;
    if count != 0 {
        return Ok(());
    }

    conn.execute("INSERT INTO file_store VALUES (?)", [path_str])?;

    let mut stmt = conn.prepare("SELECT path FROM file_store")?;
    let paths: Vec<String> = stmt
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;

    let paths: Vec<_> = paths.iter().map(|x| format!("'{}'", x)).collect();

    let sql = format!(
        "CREATE OR REPLACE VIEW ais_data AS (SELECT * FROM read_parquet([{}]))",
        paths.join(",")
    );

    conn.execute(sql.as_str(), [])?;
    Ok(())
}

pub fn update_draught(conn: &Connection, file: &Path) -> Result<(), DatabaseError> {
    let path = file.canonicalize()?;
    let path_str = path.to_string_lossy();

    let sql = format!(
        "WITH
  new_data AS (
    SELECT
      *
    FROM
      read_parquet('{}')
    WHERE
      lat != 91
  ),
  mmsis AS (
    SELECT DISTINCT
      mmsi
    FROM
      new_data
  ),
  affected_draught AS (
    SELECT * FROM (DELETE FROM draught USING (
      SELECT DISTINCT
        ON (d.mmsi) d.mmsi,
        d.draught,
        d.time_begin,
        d.duration
      FROM
        draught d,
        new_data nd
      WHERE
        nd.mmsi = d.mmsi
        AND d.time_begin < (
          SELECT
            max(timestamp)
          FROM
            new_data
          WHERE
            mmsi = d.mmsi
        )
      UNION
      SELECT DISTINCT
        ON (d.mmsi) d.mmsi,
        d.draught,
        d.time_begin,
        d.duration
      FROM
        draught d,
        new_data nd
      WHERE
        nd.mmsi = d.mmsi
        AND d.time_begin + d.duration > (
          SELECT
            min(timestamp)
          FROM
            new_data
          WHERE
            mmsi = d.mmsi
        )
    )
    RETURNING
      *
  )),
  affected_data as (
    SELECT
      mmsi,
      min(time_begin) as time_begin,
      max(time_end) as time_end
    FROM
      (
        SELECT
          mmsi,
          time_begin,
          time_begin + duration as time_end
        FROM
          affected_draught
        UNION
        SELECT
          mmsi,
          min(timestamp) as time_begin,
          max(timestamp) as time_end
        FROM
          new_data
        GROUP BY
          mmsi
      )
    GROUP BY
      mmsi
  ),
  filtered_data as (
    SELECT
      ad.*
    FROM
      ais_data ad,
      affected_data afd
    WHERE
      ad.mmsi = afd.mmsi
      AND ad.timestamp > afd.time_begin
      AND ad.timestamp < afd.time_end
  ),
  discrete_draught AS (
    SELECT DISTINCT
      ON (mmsi, timestamp) mmsi,
      timestamp,
      draught
    FROM
      filtered_data d
    ORDER BY
      mmsi,
      timestamp
  ),
  grouped_draught AS (
    SELECT
      mmsi,
      timestamp,
      draught,
      row_number() OVER (
        PARTITION BY
          mmsi
        ORDER BY
          mmsi,
          timestamp
      ) - row_number() OVER (
        PARTITION BY
          mmsi,
          draught
        ORDER BY
          mmsi,
          timestamp
      ) AS seq
    FROM
      discrete_draught
    ORDER BY
      mmsi,
      timestamp
  )
INSERT INTO
  draught
SELECT
  mmsi,
  draught,
  min(timestamp) AS time_begin,
  max(timestamp) - min(timestamp) AS duration
FROM
  grouped_draught
WHERE
  draught IS NOT NULL
GROUP BY
  mmsi,
  draught,
  seq
ORDER BY
  mmsi,
  (min(timestamp))",
        path_str
    );

    conn.execute(&sql, [])?;
    Ok(())
}
