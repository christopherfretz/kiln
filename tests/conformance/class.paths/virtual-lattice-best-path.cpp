// [class.paths]/1: if a declaration can be reached by several paths through
// a multiple inheritance graph, the access is that of the path that gives
// most access. In this lattice every path to L0a but one goes through a
// private virtual base; the one through L7b, L6b, ..., L0b is public, so
// `f` is accessible. GCC accepts it; clang 19 and 22 reject it ("'f' is a
// private member of 'L0a'"): their path search visits each virtual base
// once, through its first path, and never sees the public one
// (docs/status/deviations.md). kiln searched the first 64 of the 2^8 paths
// and rejected it too, from depth 7 on (WP-3404).
//@ skip(clang): clang's best-path search visits a virtual base through its first path only
//@ oracle: gcc
//@ expect: compile-ok
struct L0a { void f(); };
struct L0b : virtual L0a {};
struct L1a : private virtual L0a, public virtual L0b {};
struct L1b : private virtual L0a, public virtual L0b {};
struct L2a : private virtual L1a, public virtual L1b {};
struct L2b : private virtual L1a, public virtual L1b {};
struct L3a : private virtual L2a, public virtual L2b {};
struct L3b : private virtual L2a, public virtual L2b {};
struct L4a : private virtual L3a, public virtual L3b {};
struct L4b : private virtual L3a, public virtual L3b {};
struct L5a : private virtual L4a, public virtual L4b {};
struct L5b : private virtual L4a, public virtual L4b {};
struct L6a : private virtual L5a, public virtual L5b {};
struct L6b : private virtual L5a, public virtual L5b {};
struct L7a : private virtual L6a, public virtual L6b {};
struct L7b : private virtual L6a, public virtual L6b {};
struct L8a : private virtual L7a, public virtual L7b {};
struct L8b : private virtual L7a, public virtual L7b {};
void h(L8a &d) { d.f(); }
