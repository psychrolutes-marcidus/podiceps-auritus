CREATE SEQUENCE traj_seq;

CREATE TABLE trajectories (
    id INTEGER PRIMARY KEY DEFAULT nextval('traj_seq'),
    mmsi UINTEGER,
    time_begin TIMESTAMP,
    duration INTERVAL
);

CREATE OR REPLACE VIEW latest_trajectories AS (
    SELECT DISTINCT
        ON (mmsi) *
    FROM
        trajectories
    ORDER BY
        time_begin DESC
);

CREATE OR REPLACE VIEW oldest_message AS (
    SELECT DISTINCT
        ON (mmsi) mmsi,
        timestamp AS time_begin
    FROM
        ais_point
    ORDER BY
        timestamp
);
