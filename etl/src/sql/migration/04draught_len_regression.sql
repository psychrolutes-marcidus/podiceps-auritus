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

--\csv
WITH
    base AS (
        SELECT
            draught::decimal(10, 1) AS draught_y,
            draught::decimal(10, 1) - AVG(draught::decimal(10, 1)) over () AS draught_y_centered,
            ship_length::decimal(10, 1) AS ship_length_x1,
            ship_length::decimal(10, 1) - AVG(ship_length::decimal(10, 1)) AS ship_length_x1_centered
        FROM
            main.ais_data
        WHERE
            ship_length IS NOT NULL
            AND draught IS NOT NULL
    ),
    regress AS (
        SELECT
            AVG(draught_y) - avg(ship_lenght_x1) * SUM(draught_y_centered * ship_length_x1_centered) / sum(ship_length_x1_centered * ship_length_x1_centered) AS const_coef,
            SUM(draught_y_centered * ship_length_x1_centered) / SUM(ship_length_x1_centered, ship_length_x1_centered) AS x1_coef
        FROM
            base
    )
SELECT
    *
FROM
    regress;

SELECT
    draught::decimal(10, 1) AS draught_y,
    draught::decimal(10, 1) - AVG(draught::decimal(10, 1)) over () AS draught_y_centered,
    ship_length::decimal(10, 1) AS ship_length_x1,
    ship_length::decimal(10, 1) - AVG(ship_length::decimal(10, 1)) AS ship_length_x1_centered
FROM
    main.ais_data
WHERE
    ship_length IS NOT NULL
    AND draught IS NOT NULL;