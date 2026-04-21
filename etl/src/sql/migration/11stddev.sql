CREATE OR REPLACE TABLE vessel_stats.std_draught AS (
    SELECT
        mmsi,
        STDDEV_POP(draught) AS sd_draught,
        ABS(STDDEV_POP(draught) / AVG(draught)) AS rsd_avg_draught,
        ABS(STDDEV_POP(draught) / MEDIAN(draught)) AS rsd_median_draught
    FROM
        main.ais_data
    GROUP BY
        mmsi
);