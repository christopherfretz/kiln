// [dcl.type.pack.index]/1, [expr.type.conv]/1 (C++26, P2662): a
// pack-index-specifier is a simple-type-specifier, so `T...[0]{}` and
// `T...[0]()` are functional casts; kiln parsed the one-token name before
// `...[` as an expression and rejected both (WP-1029). Verified against
// clang 19.
//@ expect: run
//@ stdout: 4
extern "C" int printf(const char *, ...);
template <class... T> int f() { return T...[0]{}; }
template <class... T> int g() { return T...[0](); }
template <class... T> int h() { return sizeof(T...[0]); }
template <class... T> long k(T... t) { return T...[1](t...[0]) + T...[1]{}; }
int main() { printf("%d\n", f<int>() + g<int>() + h<int>() + int(k(0, 0L))); }
