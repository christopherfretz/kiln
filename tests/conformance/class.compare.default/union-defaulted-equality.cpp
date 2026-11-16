// [class.compare.default]/2: a defaulted comparison of a union is deleted
// only if the union has a variant member; an empty union compares equal.
// Found running GCC's C++ testsuite (WP-1497).
//@ expect: compile-ok
//@ std: c++20
union E { bool operator==(const E &) const = default; };
union G { bool operator==(const G &) const = default; static int s; void f(); };
bool h(E a, E b, G c, G d) { return a == b && c == d; }
