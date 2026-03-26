Begin;


-- might be bad if the underlying vessel changes, but MMSI does not.
CREATE OR REPLACE VIEW main.confidence_by_mmsi AS (
    SELECT
        mmsi,
        min(draught) AS mi,
        max(draught) AS ma,
        quantile_disc(draught, [0.05, 0.95]) AS confidence -- not using `quantile_cont` because it will interpolate measurements
    FROM
        main.ais_data
    GROUP BY
        mmsi
    ORDER BY
        ma - mi DESC
);

COMMIT;