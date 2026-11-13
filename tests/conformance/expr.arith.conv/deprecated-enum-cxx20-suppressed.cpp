// The -Wno- spellings that real builds pass silence the C++20 deprecations,
// through clang's group nesting too (`-Wno-enum-conversion` covers
// `deprecated-enum-enum-conversion` and `deprecated-enum-float-conversion`).
//@ std: c++20
//@ args: -Werror -Wno-deprecated-anon-enum-enum-conversion -Wno-enum-conversion -Wno-deprecated-enum-compare
//@ strict-diags
enum Q { A }; enum R { B }; enum { N = 4 };
int f(Q q, R r) { return q % N; }
int g(Q q, R r) { return q + r; }
double h(Q q) { return 1.0 / q; }
bool k(Q q, R r) { return q == r; }
int m(Q q, R r) { return true ? q : r; }
