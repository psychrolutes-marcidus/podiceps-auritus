# TODO
- [Refactor postgres extension]
- [Model linestrings as splines]
- [Distance to AIS point error]

# DOING

# DONE


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

