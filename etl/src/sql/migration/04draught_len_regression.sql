CREATE SCHEMA IF NOT EXISTS vessel_stats;

CREATE OR REPLACE VIEW vessel_stats.linear_regression AS (
    SELECT
        -- vt.mmsi,
        ship_type,
        REGR_SLOPE(draught, ship_length) AS slope, -- growth in draught as a function of ship length
        REGR_INTERCEPT(draught, ship_length) AS intercept, -- draught-offset at ship_length=0
        REGR_R2(draught, ship_length) AS r_squared,
        count(*) AS messages_count
    FROM
        main.ais_data
    WHERE
        lat != 91 -- REGR_{SLOPE | INTERCEPT | R2} ignore null values
    GROUP BY
        ship_type
);

SELECT
    -- vt.mmsi,
    ship_type,
    REGR_SLOPE(draught, ship_length) AS slope, -- growth in draught as a function of ship length
    REGR_INTERCEPT(draught, ship_length) AS intercept, -- draught-offset at ship_length=0
    REGR_R2(draught, ship_length) AS r_squared,
    count(*) AS messages_count
FROM
    main.ais_data
WHERE
    lat != 91 -- REGR_{SLOPE | INTERCEPT | R2} ignore null values
GROUP BY
    ship_type
ORDER BY
    ship_type;

/* SELECT
*
FROM
ais_data ad
JOIN main.confidence_by_vessel cbv ON ad.ship_type = cbv.ship_type
AND ad.draught >= cbv.confidence[1]
AND ad.draught <= cbv.confidence[2]; */

-- regression 
WITH
    draughts_within AS (
        SELECT
            cbv.ship_type,
            ad.ship_length,
            ad.draught,
            ad.lat
        FROM
            main.ais_data ad
            JOIN main.confidence_by_vessel cbv ON ad.ship_type = cbv.ship_type
            AND ad.draught >= cbv.confidence[1]
            AND ad.draught <= cbv.confidence[2]
    )
SELECT
    -- vt.mmsi,
    ship_type,
    REGR_SLOPE(draught::decimal(10, 1), ship_length) AS slope, -- growth in draught as a function of ship length
    REGR_INTERCEPT(draught, ship_length) AS intercept, -- draught-offset at ship_length=0
    REGR_R2(draught, ship_length) AS r_squared,
    count(*) AS messages_count
FROM
    draughts_within
WHERE
    lat != 91 -- REGR_{SLOPE | INTERCEPT | R2} ignore null values
GROUP BY
    ship_type
ORDER BY
    ship_type;