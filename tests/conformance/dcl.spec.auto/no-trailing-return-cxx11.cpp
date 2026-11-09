// [dcl.spec.auto] (C++11): a function declared with `auto` has a trailing
// return type; return type deduction is C++14 (N3638). clang
// `GetFullTypeForDeclarator`: `err_auto_missing_trailing_retty` for `auto`,
// `err_deduced_return_type` for `decltype(auto)` (whose own C++14 extension
// warning is off here), at the type specifier, for the declarator chunk
// next to the name only (WP-1123).
//@ expect: compile-fail
//@ std: c++11
//@ args: -Wno-c++14-extensions
//@ strict-diags
//@ diag: 18:1 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 19:1 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 20:8 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 22:3 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 23:10 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 25:1 error "deduced return types are a C++14 extension"

auto declared();
auto defined() { return 1; }
static auto with_storage() { return 'c'; }
struct S {
  auto member();
  static auto static_member() { return 2; }
};
decltype(auto) decltype_auto();

// Valid: a trailing return type, an `auto` variable, a pointer to a
// function with a trailing return type, a lambda.
auto trailing() -> int;
auto trailing_def() -> int { return 1; }
auto var = 1;
auto (*fp)() -> int = trailing;
auto lambda = [] { return 1; };
