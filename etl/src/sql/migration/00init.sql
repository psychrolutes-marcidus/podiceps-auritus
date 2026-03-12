CREATE TABLE file_store (path TEXT);
CREATE OR REPLACE VIEW ais_data AS (SELECT * FROM read_parquet('etl/src/sql/schema.parquet') WHERE lat != 91); -- This is a placeholder view. It will be replace by the correct view once data is loaded.
CREATE OR REPLACE VIEW ais_point AS (SELECT mmsi, timestamp, lat, lon from ais_data);
CREATE OR REPLACE VIEW draught_by_ship_type AS (SELECT DISTINCT ship_type, draught, count(*) as samples FROM ais_data WHERE draught IS NOT NULL GROUP BY ship_type, draught);
CREATE OR REPLACE VIEW draught_by_all AS (SELECT DISTINCT draught, count(*) as samples FROM ais_data WHERE draught IS NOT NULL GROUP BY draught);
CREATE OR REPLACE VIEW draught_by_mmsi AS (SELECT DISTINCT mmsi, draught, count(*) as samples FROM ais_data WHERE draught IS NOT NULL GROUP BY mmsi, draught);
