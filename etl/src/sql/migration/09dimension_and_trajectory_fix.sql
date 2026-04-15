CREATE OR REPLACE VIEW vessel_attributes.dimensions AS (
  SELECT
    mmsi,
    timestamp,
    CASE
      WHEN to_bow IS NULL
      OR to_starboard IS NULL
      OR to_stern IS NULL
      OR to_port IS NULL THEN NULL
      ELSE to_bow
    END as to_bow,
    CASE
      WHEN to_bow IS NULL
      OR to_starboard IS NULL
      OR to_stern IS NULL
      OR to_port IS NULL THEN NULL
      ELSE to_starboard
    END as to_starboard,
    CASE
      WHEN to_bow IS NULL
      OR to_starboard IS NULL
      OR to_stern IS NULL
      OR to_port IS NULL THEN NULL
      ELSE to_stern
    END as to_stern,
    CASE
      WHEN to_bow IS NULL
      OR to_starboard IS NULL
      OR to_stern IS NULL
      OR to_port IS NULL THEN NULL
      ELSE to_port
    END as to_port,
  FROM
    main.ais_data
);

DROP TABLE IF EXISTS trajectories;
