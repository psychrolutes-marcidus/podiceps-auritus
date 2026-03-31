BEGIN;

CREATE OR REPLACE VIEW main.confidence_by_vessel AS (
    SELECT
        ship_type,
        min(draught) AS mi,
        max(draught) AS ma,
        QUANTILE_DISC(draught, [0.05, 0.95]) AS confidence,
        count(draught) AS num_draughts,
        count(DISTINCT draught) AS distinct_draughts
    FROM
        main.ais_data
    GROUP BY
        ship_type
);

COMMIT;