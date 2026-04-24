CREATE OR REPLACE VIEW vessel_attributes.dimensions AS (
  SELECT
    mmsi,
    timestamp,
      to_bow,
      to_starboard,
      to_stern,
      to_port,
  FROM
    main.ais_data
);

DROP TABLE IF EXISTS trajectories;
