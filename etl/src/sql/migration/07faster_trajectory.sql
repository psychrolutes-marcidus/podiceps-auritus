CREATE TABLE newest_message (
	mmsi UINTEGER PRIMARY KEY,
	time_begin TIMESTAMP,
);

DROP VIEW IF EXISTS oldest_message;
