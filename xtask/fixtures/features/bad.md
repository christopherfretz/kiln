# Fixture: malformed rows (xtask/src/features.rs tests)

## Rows

| Paper | Feature | `__cpp_*` macro (kiln / clang 22) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| X1 | not a paper | — | done | `conformance/a.cpp` | |
| P0002R1 | bad status | — | finished | `conformance/a.cpp` | |
| P0003R1 | unquoted macro | __cpp_x 1 / 1 | done | `conformance/a.cpp` | |
| P0004R1 | too few cells | — | done | |
| P0005R1 | not a cpp macro | `FOO` 1 / 1 | done | `conformance/a.cpp` | |
| P0006R1 | bad value | `__cpp_y` 1 / soon | done | `conformance/a.cpp` | |
| P0007R1 | first | — | todo | | |
| P0007R1 | duplicate | — | todo | | |
| P0008R1 | second macro unquoted | `__cpp_a` 1 / 1<br>__cpp_b 2 / 2 | done | `conformance/a.cpp` | |
| [Bad Name] | bad stable name | — | done | `conformance/a.cpp` | |
