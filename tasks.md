# TODO
- [Model linestrings as splines]
- [Model 2D vessels as splines]
- [2D vessel rotation]
- [Compression of vessel trajectories]
- [Git Hooks pre-commit]
- [Combine cell probability with metadata]
- [Foreign tables in postgres dev environment]
- [Implement missing spatial operations in DuckDB]
- [Port extension to DuckDB]

# DOING
- [Test suite for proving results] (Anders)
- [Distance to AIS point error] (Andrzej)

# DONE
- [Refactor postgres extension] (Rasmus)

# Task Descriptions

## Refactor postgres extension
The postgres extension functions in tileheater are somewhat linked to some of the algorithmic work.
Therefore, the algorithmic part of it should be refactored out such that it is easier to test the code outside of the postgres database.
The extension should only handle convertion from WKB to internal geospatial types.

## Model linestrings as splines
Linestrings should be rendered as splines in order to emulate more accurate vessel movement.

## Distance to AIS point error
We want to measure distance between the sampled cell and the nearest AIS point in order to measures some form of error.
Please expand further.
- Distance can be measured via `LineTriangle`, same step as `point_occupation()` (method used to get occupation time for interpolated cells)
  - `distance_to_ais()` on the `ais-distance` branch measures distance from (shortest path from probe to point on line) to closest AIS point 
- When a cell is 'marked' by our line interpolation, should the distance to ais be:
  - From <center of cell,  probe point, probe point projected onto line> to <AIS point, center of AIS cell>?
    -  Center of cell to <center of AIS cell/AIS point> 'feels' the most 'correct'

## Model 2D vessels as splines
Once a linestring can be interpreted as a spline it should be straight forward to convert this into continous lines.

## 2D vessel rotation
A vessel can rotate in its path from one point to another.
The spline can interprete the larger movements, however, if a vessel reports dimensions (a,b,c,d) then the edges of the vessel will move differently from the spline and cover different areas.
Therefore, independent rotation around a vessels GPS position is necessary in order to emulate this behaviour.

## Compression of vessel trajectories
This task depend on [Model linestrings as splines] to be implemented.

Given a trajectory can be represented as a spline, is it possible to evict an AIS point given that we can interpolate that AIS point, with an error rate by it neighbors.

## Test suite for proving results
A test suite that can compare the different outputs of renderes correctly.
We should use regression test for this.

## Git Hooks pre-commit
To avoid pushing code that will not work or ruin the results in our regression tests, we should have a git hook that runs the tests locally on the machine and verify that everything is working before a commit.

## Combine cell probability with metadata
Multiple vessels will run through the same cell multiple times.
Therefore, we should be able to determine a depth from these multiple vessels with a confidence score.

There might be statistical approaches on how to do this while being able to report a confidence score.

## Foreign tables in postgres dev environment
We want to load even more data into the database.
However, it is not possible for us to store all that data on our dev machines.
Therefore, we should utilise foreign table in postgres to access data on the development server and then use that data together with the developed extension on our local machine.

## Implement missing spatial operations in DuckDB
DuckDB does not support ST_MakeLine with PointM to LinestringM.
Therefore, we will investigate if there is a workaround with PointMCollection.

Try to:
- String manipulate the WKB to convert PointMCollection into LinestringM.
- If above does not work: Parse WKB and convert it properly in Rust.

## Port extension to DuckDB
Depending on: [Implement missing spatial operations in DuckDB]

Ditch Postgres and port the extension to DuckDB.
It minimize the amount of macros and allow us to work with C types instead of having to serialize which PGRX requires.
