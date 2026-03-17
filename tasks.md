# TODO
- [Combine cell probability with metadata]
- [Calculate confidence for a cell given a vessel]

# DOING
- [Test suite for proving results] (Anders)
- [Distance to AIS point error] (Andrzej)
- [Reimplement postgres (materialized) views in DuckDB] (Anders)
- [Load depthmodel into the DuckDB Database] (Anders)
- [Port extension to DuckDB] (Rasmus)

# DONE
- [Refactor postgres extension] (Rasmus)

# Task Descriptions


## Reimplement postgres (materialized) views in DuckDB
### changes over time
- [ ] Vessel type
- [ ] transponder type ?
- [ ] vessel dimensions (over time)
- [ ] vessel offset values (i.e. a,b,c,d)

## Refactor postgres extension
The postgres extension functions in tileheater are somewhat linked to some of the algorithmic work.
Therefore, the algorithmic part of it should be refactored out such that it is easier to test the code outside of the postgres database.
The extension should only handle convertion from WKB to internal geospatial types.


## Distance to AIS point error
We want to measure distance between the sampled cell and the nearest AIS point in order to measures some form of error.
Please expand further.
- Distance can be measured via `LineTriangle`, same step as `point_occupation()` (method used to get occupation time for interpolated cells)
  - `distance_to_ais()` on the `ais-distance` branch measures distance from (shortest path from probe to point on line) to closest AIS point 
- When a cell is 'marked' by our line interpolation, should the distance to ais be:
  - From <center of cell,  probe point, probe point projected onto line> to <AIS point, center of AIS cell>?
    -  Center of cell to <center of AIS cell/AIS point> 'feels' the most 'correct'


## Test suite for proving results
A test suite that can compare the different outputs of renderes correctly.
We should use regression test for this.


## Combine cell probability with metadata
Multiple vessels will run through the same cell multiple times.
Therefore, we should be able to determine a depth from these multiple vessels with a confidence score.

There might be statistical approaches on how to do this while being able to report a confidence score.


## Port extension to DuckDB
Depending on: [Implement missing spatial operations in DuckDB]
Remove this task at completion: [Foreign tables in postgres dev environment]

Ditch Postgres and port the extension to DuckDB.
It minimize the amount of macros and allow us to work with C types instead of having to serialize which PGRX requires.

## Calculate confidence for a cell given a vessel
The system should, as per iteration 2, be able to give a confidence score of how likely a vessel draught is possible to do in a given area.
This is based on a statistical probabilistic model.

- Analyse usual behaviour for each vessel.
  - What is a normal reported draught.

## Load depthmodel into the DuckDB Database
Geodatastyrelsen has a depthmodel over the danish waters which we should have available in DuckDB
