BEGIN;

-- note: The following views do not perform any filtering on MMSI, so expect buoys and aircraft
INSTALL spatial;

LOAD spatial;

CREATE SCHEMA IF NOT EXISTS vessel_attributes;

-- note: "OR REPLACE" is used on views to ensure structure is correct (it is very cheap to create views as opposed to populated tables)
-- transponder type
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

COMMIT;

/* SELECT
*
FROM
vessel_attributes.rot
WHERE
rot IS NOT NULL
USING SAMPLE
100 rows; */
/* SELECT
*
FROM
(
PIVOT vessel_attributes.pos_type_name ON pos_type_name USING count(mmsi)
); */