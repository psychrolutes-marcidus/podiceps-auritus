DROP VIEW ais_data;
CREATE OR REPLACE TABLE ais_data (
	mmsi uinteger,
	timestamp timestamp,
	lat float,
	lon float,
	ship_length decimal(10,1),
	ship_width decimal(10,1),
	draught decimal(10,1),
	to_bow decimal(10,1),
	to_starboard decimal(10,1),
	to_stern decimal(10,1),
	to_port decimal(10,1),
	ship_type varchar
);
