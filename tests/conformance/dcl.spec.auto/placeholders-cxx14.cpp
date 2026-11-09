// [dcl.fct]/22 (C++20 abbreviated function templates), [temp.param]/4 (C++17
// placeholder template parameters): in C++14 a function parameter cannot be
// 'auto' and a non-type template parameter cannot have a placeholder type,
// while generic lambdas and deduced return types are fine (clang
// `GetDeclSpecTypeForDeclarator` selectors 0 and 8; WP-1134).
//@ expect: compile-fail
//@ std: c++14
//@ strict-diags
//@ diag: 14:7 error "'auto' not allowed in function prototype"
//@ diag: 15:8 error "'auto' not allowed in function prototype"
//@ diag: 16:10 error "'auto' not allowed in template parameter until C++17"
//@ diag: 17:10 error "'decltype(auto)' not allowed in template parameter until C++17"
//@ diag: 18:10 error "'__auto_type' not allowed in template parameter until C++17"
int q(auto x);
void r(auto (*p)() -> auto);
template<auto N> struct A {};
template<decltype(auto) N> struct B {};
template<__auto_type N> struct C {};
auto l = [](auto x) { return x; };
auto f() { return 1; }
decltype(auto) g() { return 1; }
auto k() -> auto { return 1; }
