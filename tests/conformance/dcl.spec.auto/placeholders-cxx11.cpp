// [dcl.spec.auto] (C++11 wording), [diff.cpp11.dcl.dcl]: in C++11 a function
// declared with 'auto' needs a trailing return type, 'decltype(auto)' is a
// C++14 extension, a trailing return type cannot itself be 'auto', and a
// lambda parameter cannot be 'auto' (clang `GetFullTypeForDeclarator`:
// err_auto_missing_trailing_return, err_deduced_return_type; WP-1134).
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 20:1 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 21:10 warning "'decltype(auto)' type specifier is a C++14 extension"
//@ diag: 21:1 error "deduced return types are a C++14 extension"
//@ diag: 22:1 error "'__auto_type' not allowed in function return type"
//@ diag: 23:13 error "'auto' not allowed in function return type"
//@ diag: 24:12 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 25:7 error "'auto' not allowed in function prototype"
//@ diag: 26:13 error "'auto' not allowed in lambda parameter before C++14"
//@ diag: 27:23 error "'auto' not allowed in function return type"
//@ diag: 28:22 warning "'decltype(auto)' type specifier is a C++14 extension"
//@ diag: 28:13 error "'decltype(auto)' not allowed in lambda parameter"
auto f() { return 1; }
decltype(auto) g() { return 1; }
__auto_type h() { return 1; }
auto k() -> auto { return 1; }
struct S { auto m() { return 1; } };
int q(auto x);
auto l = [](auto x) { return x; };
auto a = [](int x) -> auto { return x; };
auto b = [](decltype(auto) x) { return x; };
auto t() -> int;
auto (*p)() -> int;
auto ok = [](int x) { return x; };
