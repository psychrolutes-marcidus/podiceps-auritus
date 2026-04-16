CREATE SCHEMA IF NOT EXISTS vessel_stats;

CREATE TABLE IF NOT EXISTS main.length_confidence AS (
    SELECT
        ship_type,
        min(ship_length) AS mi,
        max(ship_length) AS ma,
        QUANTILE_DISC(ship_length, [0.01, 0.99]) AS confidence,
        count(ship_length) AS num_lengths,
        count(DISTINCT ship_length) AS distinct_lengths
    FROM
        main.ais_data
    GROUP BY
        ship_type
);

CREATE TABLE IF NOT EXISTS main.width_confidence AS (
    SELECT
        ship_type,
        min(ship_width) AS mi,
        max(ship_width) AS ma,
        QUANTILE_DISC(ship_width, [0.01, 0.99]) AS confidence,
        count(ship_width) AS num_widths,
        count(DISTINCT ship_width) AS distinct_widths
    FROM
        main.ais_data
    GROUP BY
        ship_type
);

CREATE TABLE IF NOT EXISTS vessel_stats.linear_regression AS (
    SELECT
        lc.ship_type,
        REGR_SLOPE(ad.draught, ad.ship_length) AS slope, -- growth in draught as a function of ship length
        REGR_INTERCEPT(ad.draught, ad.ship_length) AS intercept, -- draught-offset at ship_length=0
        REGR_R2(ad.draught, ad.ship_length) AS r_squared,
        count(*) num_messages
    FROM
        main.ais_data AS ad
        JOIN length_confidence lc ON lc.ship_type = ad.ship_type
        JOIN width_confidence wc ON wc.ship_type = ad.ship_type
        JOIN main.confidence_by_vessel vc ON vc.ship_type = ad.ship_type
    WHERE
        ad.ship_length BETWEEN lc.confidence[1] AND lc.confidence[2]
        AND ad.draught BETWEEN vc.confidence[1] AND vc.confidence[2]
        AND ad.ship_width BETWEEN wc.confidence[1] AND wc.confidence[2]
        AND ad.lat != 91 -- REGR_{SLOPE | INTERCEPT | R2} ignore null values
    GROUP BY
        lc.ship_type
);
