-- formula borrowed from https://ryxcommar.com/2022/09/15/multiple-linear-regression-in-sql-with-only-sum-and-avg/
WITH
    av AS (
        SELECT
            avg(draught::decimal(10, 1)) AS ad,
            avg(ship_length::decimal(10, 1)) AS al
        FROM
            main.ais_data
    ),
    base AS (
        SELECT
            draught::decimal(10, 1) AS draught_y,
            draught::decimal(10, 1) - av.ad AS draught_y_centered,
            ship_length::decimal(10, 1) AS ship_length_x1,
            ship_length::decimal(10, 1) - av.al AS ship_length_x1_centered
        FROM
            main.ais_data,
            av
        WHERE
            ship_length IS NOT NULL
            AND draught IS NOT NULL
    ),
    regress AS (
        SELECT
            AVG(draught_y) - avg(ship_length_x1) * SUM(draught_y_centered * ship_length_x1_centered) / sum(ship_length_x1_centered * ship_length_x1_centered) AS const_coef,
            SUM(draught_y_centered * ship_length_x1_centered) / SUM(ship_length_x1_centered * ship_length_x1_centered) AS x1_coef
        FROM
            base
    )
SELECT
    *
FROM
    regress;

-- TODO: use regr_slope instead, it might do the same
SELECT
    REGR_SLOPE(draught, ship_length) AS slope,
    REGR_INTERCEPT(draught, ship_length) AS intercept,
    REGR_R2(draught, ship_length) AS r_squared
FROM
    main.ais_data
WHERE
    draught IS NOT NULL
    AND ship_length IS NOT NULL
    AND lon != 91
    AND lat != 91;

SELECT DISTINCT
    ais_nav_status
FROM
    vessel_attributes.nav_status;

SELECT
    -- vt.mmsi,
    vt.ship_type,
    REGR_SLOPE(d.draught, sd.to_bow + sd.to_stern)
FROM
    vessel_attributes.vessel_type AS vt
    JOIN vessel_attributes.dimensions sd ON vt.mmsi = sd.mmsi
    AND vt.timestamp = vt.timestamp
    JOIN vessel_attributes.draught d ON vt.mmsi = d.mmsi
    AND vt.timestamp = d.timestamp
GROUP BY
    vt.ship_type limit 10;

--\nyt forsøg
-- csv
COPY (
    SELECT
        draught,
        ship_length
    FROM
        main.ais_data
    WHERE
        draught IS NOT NULL
        AND ship_length IS NOT NULL
        AND lon != 91
        AND lat != 91
    USING SAMPLE
        1_000_000
) TO 'draught_len.csv';

SELECT
    draught,
    ship_length
FROM
    main.ais_data
WHERE
    draught IS NOT NULL
    AND ship_length IS NOT NULL
    AND lon != 91
    AND lat != 91
    AND transponder_type = 'class a'
    AND is_valid_mmsi;