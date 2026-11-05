# Fixture: a well-formed feature table (xtask/src/features.rs tests)

## First

| Paper | Feature | `__cpp_*` macro (kiln / clang 22) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| P1938R3 | `if consteval` | `__cpp_if_consteval` 202106L / 202106L | done | `conformance/a.cpp`<br>`conformance/b.cpp` | |
| P0847R7 | Deducing `this` | — | partial | `conformance/missing.cpp` | notes |
| P2893R3 | Variadic friends | `__cpp_variadic_friend` — / 202403L | todo | `conformance/c.cpp` | |

Text between tables.

| Paper | Feature | `__cpp_*` macro (kiln / clang 22) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| P1967R14 | `#embed` | - | todo | | a `a|b` span and an escaped \| pipe |

## C++11

| Paper | Feature | `__cpp_*` macro (kiln / clang 22 at `-std=c++11`) | Status | Corpus tests | Notes |
|---|---|---|---|---|---|
| N2442 | Raw and Unicode string literals | `__cpp_raw_strings` 200710 / 200710<br>`__cpp_unicode_literals` 200710 / 200710 | done | `conformance/a.cpp` | |
| N2756 | Default member initializers | — | untested | | |
| [except] | Exceptions | — | done | `conformance/a.cpp` | |
