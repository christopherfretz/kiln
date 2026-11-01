// [temp.variadic]/5, [dcl.init.aggr]/3.1: a pack expansion of a designated
// initializer-clause (`.m = e...`, `[I] = e...`) expands to one designated
// clause per element. To the C99-extension checks on the written list the
// expansion is not a designated clause (clang `ActOnInitList`).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 14:57 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 14:65 note "first non-designated initializer is here"
//@ diag: 15:68 warning "mixture of designated and non-designated initializers in the same initializer list is a C99 extension"
//@ diag: 15:57 note "first non-designated initializer is here"
struct Pt { int x; int y; int z; };
template<int... I> constexpr int f() { int a[3]{[I] = I + 1 ...}; return a[0] + a[1] * 10 + a[2] * 100; }
static_assert(f<0, 2>() == 301);
template<class... U> constexpr Pt g(U... u) { return Pt{.x = 1, .y = u...}; }
template<class... U> constexpr Pt h(U... u) { return Pt{.y = u..., .z = 1}; }
template<class... U> constexpr Pt k(U... u) { return Pt{.y = u..., 2}; }
static_assert(g(5).y == 5 && h(6).y == 6 && h(6).z == 1 && k(7).z == 2);
