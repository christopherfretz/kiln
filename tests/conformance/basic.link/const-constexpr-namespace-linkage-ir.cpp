// [basic.link]/3.2, [dcl.constexpr]/1 (C++11 to C++26): a `const` or
// `constexpr` variable at namespace scope has internal linkage unless
// declared `extern`; one first declared `extern` keeps external linkage
// at its `constexpr` definition, a strong one; none is inline (the
// namespace-scope neighbour of the static data member rule, WP-1517).
// Compared with clang's IR.
//@ expect: ir
//@ std: c++14
extern const int x;
constexpr int x = 3;
extern const int y = 3;
constexpr int z = 3;
const int *pz = &z;
const int *get_x() { return &x; }
