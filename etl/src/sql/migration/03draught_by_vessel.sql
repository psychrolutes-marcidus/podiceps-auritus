CREATE OR REPLACE VIEW vessel_attributes.draught_nulls_by_ship_type AS (
    SELECT
        ship_type,
        (COUNT(draught) / count())::double AS draughts_null, -- count(draught) counts non nulls, count() counts total including nulls
    FROM
        ais_data
    GROUP BY
        ship_type
);