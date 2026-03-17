use crate::errors::DatabaseError;
use crate::tables::trajectories::Trajectories;
use crate::tables::*;
use chrono::TimeDelta;
use itertools::Itertools;
use linesonmaps::algo::segmenter::TrajectorySplit;
use linesonmaps::types::linestringm::LineStringM;
use postgres::types::Type;
use postgres::{Client, Config, NoTls, Statement, Transaction};
use std::collections::HashSet;
use std::io::Write;
use wkb::reader::read_wkb;

pub struct DbConn {
    pub conn: Client,
}

impl DbConn {
    pub fn new() -> Result<Self, DatabaseError> {
        let hostname: String = std::env::var("DB_HOSTNAME")?;
        let port_str: String = std::env::var("DB_PORT")?;
        let port_i: u16 = port_str.parse::<u16>()?;
        let user: String = std::env::var("DB_USERNAME")?;
        let password: String = std::env::var("DB_PASSWORD")?;
        let db_name: String = std::env::var("DB_NAME")?;

        let mut db_conf = Config::new();

        db_conf.host(&hostname);
        db_conf.port(port_i);
        db_conf.user(&user);
        db_conf.password(password);
        db_conf.dbname(&db_name);

        let client = db_conf.connect(NoTls).map_err(DatabaseError::Connect)?;
        Ok(Self { conn: client })
    }

    pub fn fetch_data(
        &mut self,
        time_begin: DateTime<Utc>,
        time_end: DateTime<Utc>,
    ) -> Result<Ships, DatabaseError> {
        let traj = fetch_trajectories(&mut self.conn, time_begin, time_end)?;

        let unique_mmsi: HashSet<i32> = traj.mmsi.iter().copied().collect();

        let unique_mmsi_vec: Vec<i32> = unique_mmsi.into_iter().collect();
        let nav = fetch_nav_status(&mut self.conn, time_begin, time_end)?;
        let draught = fetch_draught(&mut self.conn, time_begin, time_end)?;
        let cog = fetch_cog(&mut self.conn, time_begin, time_end)?;
        let dimensions = fetch_dimensions(&mut self.conn, &unique_mmsi_vec)?;
        let gps_position = fetch_gps_position(&mut self.conn, &unique_mmsi_vec)?;
        let rot = fetch_rot(&mut self.conn, time_begin, time_end)?;
        let sog = fetch_sog(&mut self.conn, time_begin, time_end)?;

        Ok(Ships {
            nav_status: nav,
            ship_draught: draught,
            cog,
            sog,
            rot,
            gps_position,
            dimensions,
            trajectories: traj,
        })
    }
}

#[allow(clippy::type_complexity)]
pub fn insert_sub_traj_inteval(
    conn: &mut Client,
    split_intervals: Vec<(i32, Vec<(DateTime<Utc>, TimeDelta)>)>,
) -> Result<Transaction<'_>, DatabaseError> {
    let temp_table = "create temp table temp_split_interval
    (
        mmsi integer,
        t_start double precision,
        t_end double precision
    )
        on commit drop";

    let mut t = conn.transaction().map_err(|e| DatabaseError::QueryError {
        db_error: e,
        msg: "could not begin transaction".into(),
    })?;

    let _ = t
        .execute(temp_table, &[])
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: "could not create temporary table".into(),
        });

    let mut writer = t
        .copy_in("COPY temp_split_interval FROM STDIN;")
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: "COPY IN".into(),
        })?;

    let _num_splits = split_intervals.len();
    let in_str = split_intervals
        .into_iter()
        .map(|g| {
            g.1.into_iter()
                .map(move |(tstz, i)| {
                    format!(
                        "{0}\t{1}\t{2}\n",
                        g.0,
                        tstz.timestamp_millis() as f64 / 1000_f64,
                        i.as_seconds_f64()
                    )
                })
                .join("")
        })
        .join("");

    writer.write_all(&in_str.into_bytes())?;
    let _count = writer.finish().expect("tokio_postgres be trolling");
    // debug_assert_eq!(
    //     count as usize, num_splits,
    //     "#copied rows should equal to #input rows"
    // );

    let insert = "insert into program_data.sub_traj_interval (mmsi, t_start, t_end)
        select mmsi, to_timestamp(t_start) as t_start, make_interval(secs => t_end) as t_end from temp_split_interval
            on conflict do nothing;";

    let _b = t
        .execute(insert, &[])
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: "failed to move from temp table to sub_traj_inteval".into(),
        })?;
    // debug_assert!(num_splits >= b.try_into().unwrap());

    Ok(t)
}

#[allow(unused)]
fn insert_split_traj<const CRS: u64>(
    conn: &mut Client,
    sub_trajs: Vec<(i32, TrajectorySplit<CRS>, f64, TimeDelta)>,
) -> Result<Transaction<'_>, DatabaseError> {
    let temp_table = "create temp table temp_split 
    (
        mmsi integer, 
        dist_thres double precision, 
        time_thres_s double precision, 
        bytev text
    ) 
    on commit drop;";

    let mut t = conn.transaction().map_err(|e| DatabaseError::QueryError {
        db_error: e,
        msg: "could not begin transaction".into(),
    })?;

    let _ = t
        .execute(temp_table, &[])
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: "could not create temp table".into(),
        })?;

    let mut writer =
        t.copy_in("COPY temp_split FROM STDIN")
            .map_err(|e| DatabaseError::QueryError {
                db_error: e,
                msg: "COPY IN".into(),
            })?;

    let sub_trajs_len = sub_trajs.len();
    let in_str = sub_trajs
        .into_iter()
        .map(|v| {
            format!(
                "{0}\t{2}\t{3}\t{1}",
                v.0,
                hex::encode_upper(v.1.to_wkb()),
                v.2,
                v.3.as_seconds_f64(),
            )
        })
        .join("\n");

    writer.write_all(&in_str.into_bytes())?;
    let count = writer.finish().expect("fuck tokio_postgres"); //TODO also add error variant
    debug_assert_eq!(
        count as usize, sub_trajs_len,
        "copied #rows should be equal to input rows"
    );

    let insert = "insert into program_data.trajectory_splits (mmsi, dist_thres, time_thres, sub_traj)
        select mmsi, dist_thres, make_interval(secs=>time_thres_s) as time_thres, st_geomfromwkb(decode(bytev,'hex'),4326) as sub_traj from temp_split
            on conflict do nothing";

    let b = t
        .execute(insert, &[])
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: "failed to insert".into(),
        })?;
    debug_assert!(sub_trajs_len >= b.try_into().unwrap()); // some trajectories might get dropped 

    // let _ = t.commit().map_err(|e| DatabaseError::QueryError {
    //     db_error: e,
    //     msg: "failed to commit transaction".into(),
    // }); // temp table should be dropped by this
    Ok(t)
}

fn fetch_nav_status(
    conn: &mut Client,
    time_begin: DateTime<Utc>,
    time_end: DateTime<Utc>,
) -> Result<nav_status::NavStatus, DatabaseError> {
    let mut nav_status_table: nav_status::NavStatus = nav_status::NavStatus::new();
    let result = conn
        .query(
            "SELECT mmsi, time_begin, time_end, status_name
                FROM PROGRAM_DATA.NAV_STATUS
                WHERE
                    time_end >= $1 AND time_begin <= $2",
            &[&time_begin, &time_end],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("nav_status_query"),
        })?;

    let size = result.len();
    nav_status_table.mmsi.reserve(size);
    nav_status_table.time_begin.reserve(size);
    nav_status_table.time_end.reserve(size);
    nav_status_table.nav_status.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let time_begin: DateTime<Utc> = row.get("time_begin");
        let time_end: DateTime<Utc> = row.get("time_end");
        let status: String = row.get("status_name");

        let status_parsed = nav_status::nav_status_converter(&status);
        nav_status_table.mmsi.push(mmsi);
        nav_status_table.time_begin.push(time_begin);
        nav_status_table.time_end.push(time_end);
        nav_status_table.nav_status.push(status_parsed);
    }

    Ok(nav_status_table)
}

fn fetch_draught(
    conn: &mut Client,
    time_begin: DateTime<Utc>,
    time_end: DateTime<Utc>,
) -> Result<ship_draught::Draught, DatabaseError> {
    let mut draught_table: ship_draught::Draught = ship_draught::Draught::new();

    let result = conn
        .query(
            "SELECT mmsi, time_begin, time_end, draught
                            FROM
                                PROGRAM_DATA.DRAUGHT
                            WHERE
                                TIME_END >= $1 AND TIME_BEGIN <= $2",
            &[&time_begin, &time_end],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("draught query"),
        })?;

    let size = result.len();

    draught_table.mmsi.reserve(size);
    draught_table.time_begin.reserve(size);
    draught_table.time_end.reserve(size);
    draught_table.draught.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let time_begin: DateTime<Utc> = row.get("time_begin");
        let time_end: DateTime<Utc> = row.get("time_end");
        let draught: f32 = row.get("draught");

        draught_table.mmsi.push(mmsi);
        draught_table.time_begin.push(time_begin);
        draught_table.time_end.push(time_end);
        draught_table.draught.push(draught);
    }

    Ok(draught_table)
}

fn fetch_cog(
    conn: &mut Client,
    time_begin: DateTime<Utc>,
    time_end: DateTime<Utc>,
) -> Result<cog::Cog, DatabaseError> {
    let mut cog_table: cog::Cog = cog::Cog::new();

    let result = conn
        .query(
            "SELECT mmsi, timestamp, cog
            FROM	PROGRAM_DATA.COG
            WHERE timestamp >= $1 AND timestamp <= $2",
            &[&time_begin, &time_end],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("cog query"),
        })?;

    let size = result.len();

    cog_table.mmsi.reserve(size);
    cog_table.time.reserve(size);
    cog_table.cog.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let time: DateTime<Utc> = row.get("timestamp");
        let cog: f32 = row.get("cog");
        cog_table.mmsi.push(mmsi);
        cog_table.time.push(time);
        cog_table.cog.push(cog);
    }

    Ok(cog_table)
}

fn fetch_sog(
    conn: &mut Client,
    time_begin: DateTime<Utc>,
    time_end: DateTime<Utc>,
) -> Result<sog::Sog, DatabaseError> {
    let mut sog_table: sog::Sog = sog::Sog::new();

    let result = conn
        .query(
            "SELECT mmsi, timestamp, sog
        FROM PROGRAM_DATA.SOG
        WHERE timestamp >= $1 AND timestamp <= $2",
            &[&time_begin, &time_end],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("sog query"),
        })?;

    let size = result.len();

    sog_table.mmsi.reserve(size);
    sog_table.time.reserve(size);
    sog_table.sog.reserve(size);

    for (i, row) in result.iter().enumerate() {
        let mmsi: i32 = row.get("mmsi");
        let time: DateTime<Utc> = row.get("timestamp");
        let sog: f32 = row.get("sog");
        sog_table.mmsi.push(mmsi);
        sog_table.time.push(time);
        sog_table.sog.push(sog);
        sog_table.b_tree_index.insert((mmsi, time), i);
    }
    Ok(sog_table)
}

fn fetch_rot(
    conn: &mut Client,
    time_begin: DateTime<Utc>,
    time_end: DateTime<Utc>,
) -> Result<rot::Rot, DatabaseError> {
    let mut rot_table: rot::Rot = rot::Rot::new();

    let result = conn
        .query(
            "SELECT mmsi, timestamp, rot
    FROM PROGRAM_DATA.ROT
    WHERE timestamp >= $1 AND timestamp <= $2",
            &[&time_begin, &time_end],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("rot query"),
        })?;

    let size = result.len();

    rot_table.mmsi.reserve(size);
    rot_table.time.reserve(size);
    rot_table.rot.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let time: DateTime<Utc> = row.get("timestamp");
        let rot: f32 = row.get("rot");
        rot_table.mmsi.push(mmsi);
        rot_table.time.push(time);
        rot_table.rot.push(rot);
    }

    Ok(rot_table)
}

fn fetch_trajectories(
    conn: &mut Client,
    time_begin: DateTime<Utc>,
    time_end: DateTime<Utc>,
) -> Result<trajectories::Trajectories, DatabaseError> {
    let mut trajectories_table = trajectories::Trajectories::new();

    let result = conn
        .query(
            "SELECT mmsi, ST_AsBinary(ST_FilterByM(traj, $1, $2, true), 'NDR') as traj
FROM PROGRAM_DATA.trajectories
WHERE ST_IsEmpty(ST_FilterByM(traj, $1, $2)) = false;",
            &[
                &(time_begin.timestamp() as f64),
                &(time_end.timestamp() as f64),
            ],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("trajectories query"),
        })?;

    let size = result.len();

    trajectories_table.mmsi.reserve(size);
    trajectories_table.trajectory.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let traj: Vec<u8> = row.get("traj");

        let wkb_data = read_wkb(traj.as_slice())?;
        let lsm = LineStringM::try_from(wkb_data)?;

        trajectories_table.mmsi.push(mmsi);
        trajectories_table.trajectory.push(lsm);
    }

    Ok(trajectories_table)
}

fn fetch_dimensions(
    conn: &mut Client,
    mmsi: &[i32],
) -> Result<dimensions::Dimensions, DatabaseError> {
    let mut dimensions_table = dimensions::Dimensions::new();

    let result = conn
        .query(
            "SELECT mmsi, width, length
FROM program_data.dimensions
WHERE mmsi = ANY($1)",
            &[&mmsi],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("dimensions query"),
        })?;

    let size = result.len();

    dimensions_table.mmsi.reserve(size);
    dimensions_table.width.reserve(size);
    dimensions_table.length.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let width: f64 = row.get("width");
        let length: f64 = row.get("length");

        dimensions_table.mmsi.push(mmsi);
        dimensions_table.width.push(width);
        dimensions_table.length.push(length);
    }

    Ok(dimensions_table)
}

fn fetch_gps_position(
    conn: &mut Client,
    mmsi: &[i32],
) -> Result<gps_position::GPSPosition, DatabaseError> {
    let mut gps_position_table = gps_position::GPSPosition::new();

    let result = conn
        .query(
            "SELECT mmsi, a, b, c, d
FROM program_data.gps_position
WHERE mmsi = ANY($1)",
            &[&mmsi],
        )
        .map_err(|e| DatabaseError::QueryError {
            db_error: e,
            msg: String::from("gps_position query"),
        })?;

    let size = result.len();

    gps_position_table.mmsi.reserve(size);
    gps_position_table.a.reserve(size);
    gps_position_table.b.reserve(size);
    gps_position_table.c.reserve(size);
    gps_position_table.d.reserve(size);

    for row in result {
        let mmsi: i32 = row.get("mmsi");
        let a: f64 = row.get("a");
        let b: f64 = row.get("b");
        let c: f64 = row.get("c");
        let d: f64 = row.get("d");

        gps_position_table.mmsi.push(mmsi);
        gps_position_table.a.push(a);
        gps_position_table.b.push(b);
        gps_position_table.c.push(c);
        gps_position_table.d.push(d);
    }

    Ok(gps_position_table)
}

pub struct TrajectoryIter<const CHUNK_SIZE: u32> {
    conn: DbConn,
    offset: u32,
    statement: Statement,
}

impl<const CHUNK_SIZE: u32> TrajectoryIter<CHUNK_SIZE> {
    pub fn new(conn: DbConn) -> Result<Self, DatabaseError> {
        let mut conn = conn;
        let statement = conn
            .conn
            .prepare_typed(
                "
                SELECT MMSI, st_asbinary(TRAJ,'NDR') as traj FROM
                    PROGRAM_DATA.TRAJECTORIES 
                        ORDER BY MMSI
                        LIMIT $1 
                        OFFSET $2;",
                &[Type::INT8, Type::INT8],
            )
            .map_err(|e| DatabaseError::QueryError {
                db_error: e,
                msg: "error in preparing statement".into(),
            })?;
        Ok(TrajectoryIter {
            conn,
            offset: 0,
            statement,
        })
    }
}
impl<const CHUNK_SIZE: u32> Iterator for TrajectoryIter<CHUNK_SIZE> {
    type Item = Result<Trajectories, DatabaseError>;

    fn next(&mut self) -> Option<Self::Item> {
        //! Probably doesn't behave well if view changes in between calls
        let result = self
            .conn
            .conn
            .query(
                &self.statement,
                &[&(CHUNK_SIZE as i64), &(self.offset as i64)],
            )
            .map_err(|e| DatabaseError::QueryError {
                db_error: e,
                msg: "trajectories query".into(),
            });
        let o_result = result
            .and_then(|v| {
                v.into_iter()
                    .map(|r| {
                        Ok::<(i32, LineStringM<4326>), DatabaseError>((
                            r.get::<'_, _, i32>("mmsi"),
                            LineStringM::try_from(read_wkb(
                                r.get::<'_, _, Vec<u8>>("traj").as_slice(),
                            )?)?,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .map(|v| {
                let uz = v.into_iter().unzip::<i32, LineStringM, Vec<_>, Vec<_>>();
                if !uz.0.is_empty() {
                    Some(Trajectories {
                        mmsi: uz.0,
                        trajectory: uz.1,
                    })
                } else {
                    None
                }
            })
            .transpose();
        self.offset += CHUNK_SIZE;
        o_result
    }
}

#[cfg(test)]
mod tests {
    use linesonmaps::types::pointm::PointM;

    use super::*;

    #[test]
    fn trajectories_does_not_crash() {
        dotenvy::dotenv().unwrap();

        let mut db = DbConn::new().unwrap();

        let from =
            DateTime::parse_from_str("2024-01-01 00:00:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
                .unwrap();
        let to = DateTime::parse_from_str("2024-01-01 01:00:00 +0000", "%Y-%m-%d %H:%M:%S%.3f %z")
            .unwrap();

        db.fetch_data(from.into(), to.into()).unwrap();
    }

    // takes a while to run (~100 seconds, less in release)
    #[test]
    fn trajectory_iter_works() {
        dotenvy::dotenv().unwrap();
        let mut conn = DbConn::new().unwrap();
        const SIZE: u32 = 500;
        let count = conn
            .conn
            .query(
                "select count(*) as count from program_data.trajectories",
                &[],
            )
            .unwrap()
            .first()
            .unwrap()
            .get::<'_, _, i64>("count") as u32;
        let it = TrajectoryIter::<SIZE>::new(conn)
            .expect("error in establishing connection or in preparing statement");

        assert_eq!(count.div_ceil(SIZE), it.count() as u32);
    }

    #[test]
    #[ignore = "pending deprecation, but it still works, probably :))"]
    fn split_traj_insertion_works() {
        dotenvy::dotenv().unwrap();
        let mut db = DbConn::new().unwrap();

        let ts = TrajectorySplit::<4326>::Point(PointM::from((1., 2., 3.5)));
        dbg!(hex::encode(ts.clone().to_wkb()));
        const INTERVAL: TimeDelta = TimeDelta::new(69, 0).unwrap();
        const HEX: u32 = 0xd1070000;
        const HEXX: u32 = 0x0000701d_u32;
        const HEXXX_32: u32 = 0x01000040;
        const A: u32 = 0x00_00_07_d1;
        dbg!(hex::encode(2001_u32.to_le_bytes()));
        let t = insert_split_traj(&mut db.conn, vec![(123456789, ts, 42.0, INTERVAL)])
            .expect("transaction should not fail");
        t.rollback().expect("error during rollback");
    }
    #[test]
    fn split_traj_intervals_works() {
        dotenvy::dotenv().unwrap();
        let mut db = DbConn::new().unwrap();
        let split_intevals = vec![(
            123456789,
            vec![(
                DateTime::from_timestamp_secs(1759231496).unwrap(),
                TimeDelta::new(100, 0).unwrap(),
            )],
        )];
        let t = insert_sub_traj_inteval(&mut db.conn, split_intevals).unwrap();
        t.rollback().expect("error during rollback");
        // t.commit().unwrap();
    }
}
