# TODO
- [Refactor postgres extension]

# DOING

# DONE


# Task Descriptions

## Refactor postgres extension
The postgres extension functions in tileheater are somewhat linked to some of the algorithmic work.
Therefore, the algorithmic part of it should be refactored out such that it is easier to test the code outside of the postgres database.
The extension should only handle convertion from WKB to internal geospatial types.

