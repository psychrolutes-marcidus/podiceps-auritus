CREATE OR REPLACE TABLE vessel_stats.draught_histogram_by_ship_types AS (
    SELECT
        ship_type,
        histogram(draught) AS "histogram" -- key: draught, value: #measurements
    FROM
        ais_data
    GROUP BY
        ship_type
);