SELECT
    ship_type,
    unnest(
        map_entries(histogram(draught)),
        recursive := TRUE
    )
FROM
    ais_data
GROUP BY
    ship_type;