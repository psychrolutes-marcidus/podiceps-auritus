INSTALL spatial;

LOAD spatial;


CREATE TABLE file_store (path TEXT);
CREATE OR REPLACE VIEW ais_data AS (SELECT * FROM read_parquet('etl/src/sql/schema.parquet') WHERE lat != 91); -- This is a placeholder view. It will be replace by the correct view once data is loaded.
CREATE OR REPLACE VIEW ais_point AS (SELECT mmsi, timestamp, lat, lon from ais_data);
CREATE OR REPLACE VIEW draught_by_ship_type AS (SELECT DISTINCT ship_type, draught, count(*) as samples FROM ais_data WHERE draught IS NOT NULL GROUP BY ship_type, draught);
CREATE OR REPLACE VIEW draught_by_all AS (SELECT DISTINCT draught, count(*) as samples FROM ais_data WHERE draught IS NOT NULL GROUP BY draught);
CREATE OR REPLACE VIEW draught_by_mmsi AS (SELECT DISTINCT mmsi, draught, count(*) as samples FROM ais_data WHERE draught IS NOT NULL GROUP BY mmsi, draught);

CREATE SCHEMA IF NOT EXISTS vessel_attributes;

CREATE OR REPLACE VIEW vessel_attributes.transponder AS
SELECT
    mmsi,
    "timestamp",
    transponder_type
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.transponder IS 'transponder type (e.g. class A, class B)';

-- includes NULLs and vessel type "undefined"
CREATE OR REPLACE VIEW vessel_attributes.vessel_type AS
SELECT
    mmsi,
    "timestamp",
    ship_type
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.vessel_type IS 'vessel type (e.g. fishing, cargo)';

-- UoM: meter
CREATE OR REPLACE VIEW vessel_attributes.dimensions AS
SELECT
    mmsi,
    "timestamp",
    to_bow, -- front
    to_starboard, -- right
    to_stern, -- rear
    to_port -- left
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.dimensions IS 'vessel dimensions (bow=front,starboard=right,stern=rear,port=left)';

-- UoM: meter
CREATE OR REPLACE VIEW vessel_attributes.width_length AS
SELECT
    mmsi,
    "timestamp",
    ship_width,
    ship_length
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.width_length IS 'vessel width and length';

CREATE OR REPLACE VIEW vessel_attributes.sog AS
SELECT
    mmsi,
    "timestamp",
    sog,
    (sog::double * 0.5144444444) AS sog_ms
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.sog IS 'speed over ground (SOG)';

CREATE OR REPLACE VIEW vessel_attributes.cog AS
SELECT
    mmsi,
    "timestamp",
    cog
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.cog IS 'course over ground (COG) (min/max 0/360)';

--note: ROT is expressed in degrees/min
CREATE OR REPLACE VIEW vessel_attributes.rot AS
SELECT
    mmsi,
    "timestamp",
    rot
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.rot IS 'rate of turn in degrees/min (min/max -720/720)';

CREATE OR REPLACE VIEW vessel_attributes.nav_status AS
SELECT
    mmsi,
    "timestamp",
    ais_nav_status
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.nav_status IS 'navigational status';

CREATE OR REPLACE VIEW vessel_attributes.pos_type_name AS
SELECT
    mmsi,
    "timestamp",
    pos_type_name
FROM
    main.ais_data;

COMMENT ON VIEW vessel_attributes.pos_type_name IS 'method used for obtaining position';

CREATE OR REPLACE VIEW vessel_attributes.draught AS (
    SELECT mmsi, "timestamp", draught
    FROM main.ais_data
);

COMMENT ON VIEW vessel_attributes.draught IS 'draught reported by all vessels in a single view';
