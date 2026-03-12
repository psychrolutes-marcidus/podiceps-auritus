BEGIN;
CREATE TABLE file_store (path TEXT);
CREATE OR REPLACE VIEW ais_data AS (SELECT * FROM read_parquet('etl/src/sql/schema.parquet') WHERE lat != 91); -- This is a placeholder view. It will be replace by the correct view once data is loaded.
CREATE OR REPLACE VIEW ais_point AS (SELECT mmsi, timestamp, lat, lon from ais_data);
CREATE OR REPLACE VIEW draught AS (SELECT mmsi, timestamp, draught from ais_data);
-- CREATE OR REPLACE VIEW draught AS (
--     WITH discrete_draught AS (
--         SELECT DISTINCT ON (mmsi, timestamp) mmsi, timestamp, draught
--         FROM ais_data d
--         ORDER BY mmsi, timestamp
--     ), grouped_draught AS (
--         SELECT mmsi, timestamp, draught, row_number() OVER (PARTITION BY mmsi ORDER BY mmsi, timestamp) - row_number() OVER (PARTITION BY mmsi, draught ORDER BY mmsi, timestamp) AS seq
--         FROM discrete_draught
--         ORDER BY mmsi, timestamp
--     )
--     SELECT mmsi, draught, min(timestamp) AS time_begin, max(timestamp) - min(timestamp) AS duration
--     FROM grouped_draught
--     WHERE draught IS NOT NULL
--     GROUP BY mmsi, draught, seq
--     ORDER BY mmsi, (min(timestamp))
-- ); -- Turn this into some form of materialized view.

-- CREATE OR REPLACE VIEW gps_position AS (
--     WITH duplicate_count AS (
--         SELECT DISTINCT mmsi, a, b, c, d, count(*) AS count
--         FROM ais_data
--         GROUP BY mmsi, a, b, c, d
--         ORDER BY (count(*)) DESC
--     )
--     SELECT DISTINCT ON (mmsi), mmsi, a, b, c, d
--     FROM duplicate_count
-- )
COMMIT;
