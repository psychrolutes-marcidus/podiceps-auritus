CREATE OR REPLACE VIEW main.within_confidence_by_vessel_type AS (
    SELECT
        ad.* -- remember to filter out unwanted columns, or risk long query times
    FROM
        main.ais_data ad
        JOIN main.confidence_by_vessel cbv ON ad.ship_type = cbv.ship_type
        AND ad.draught >= cbv.confidence[1]
        AND ad.draught <= cbv.confidence[2]
);

COMMENT ON VIEW main.within_confidence_by_vessel_type IS 'rows where draught measurement is within confidence interval of respective ship type';

CREATE OR REPLACE VIEW main.within_confidence_by_mmsi AS (
    SELECT
        ad.* -- remember to filter out unwanted columns, or risk long query times
    FROM
        main.ais_data ad
        JOIN main.confidence_by_mmsi cbm ON ad.mmsi = cbm.mmsi
        AND ad.draught >= cbm.confidence[1]
        AND ad.draught <= cbm.confidence[2]
);

COMMENT ON VIEW main.within_confidence_by_vessel_type IS 'rows where draught measurement is within confidence interval of respective mmsi';