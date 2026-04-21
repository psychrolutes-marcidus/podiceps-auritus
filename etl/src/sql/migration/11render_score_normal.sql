CREATE SCHEMA IF NOT EXISTS render;
CREATE OR REPLACE VIEW render.draught_dist_mmsi_normal AS
    (
SELECT DISTINCT
  ad.mmsi,
  ad.draught,
  CASE
    WHEN ad.draught >= cbm.confidence[1]
    AND ad.draught <= cbm.confidence[2] THEN 1::float
    ELSE least(
      abs((ad.draught - mi) / (cbm.confidence[1] - mi)),
      abs((ad.draught - ma) / (cbm.confidence[2] - ma))
    )::float
  END AS score_norm,
FROM
  ais_data ad
  LEFT JOIN confidence_by_mmsi cbm ON ad.mmsi = cbm.mmsi
);

CREATE OR REPLACE VIEW render.draught_dist_vessel_type_normal AS
    (
SELECT DISTINCT ON (ad.mmsi, ad.draught)
  ad.mmsi,
  ad.draught,
  CASE
    WHEN ad.draught >= cbv.confidence[1]
    AND ad.draught <= cbv.confidence[2] THEN 1::float
    ELSE least(
      abs((ad.draught - mi) / (cbv.confidence[1] - mi)),
      abs((ad.draught - ma) / (cbv.confidence[2] - ma))
    )::float
  END AS score_norm,
  cbv.*
FROM
  ais_data ad
  LEFT JOIN confidence_by_vessel cbv ON ad.ship_type = cbv.ship_type
);
