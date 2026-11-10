// [dcl.spec.auto] (C++11 wording): in a template, a function declared with
// a written `auto`, `decltype(auto)` or `__auto_type` return type is still
// rejected, as are a lambda's `auto` parameter and placeholder trailing
// return types; instantiating the enclosing template adds no diagnostic,
// and a lambda without a trailing return type (an implied `auto`,
// [expr.prim.lambda.closure]/3) is accepted there (WP-1123, WP-1134,
// WP-1213).
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 22:20 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 24:3 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 25:10 error "'auto' return without trailing return type; deduced return types are a C++14 extension"
//@ diag: 27:29 warning "'decltype(auto)' type specifier is a C++14 extension"
//@ diag: 27:20 error "deduced return types are a C++14 extension"
//@ diag: 28:20 error "'__auto_type' not allowed in function return type"
//@ diag: 30:15 error "'auto' not allowed in lambda parameter before C++14"
//@ diag: 31:23 error "'auto' not allowed in function return type"
//@ diag: 32:23 error "'__auto_type' not allowed in function return type"
//@ diag: 33:32 warning "'decltype(auto)' type specifier is a C++14 extension"
//@ diag: 33:23 error "'decltype(auto)' not allowed in function return type"
template <class S> auto f() { return S(); }
template <class S> struct C {
  auto m() { return S(); }
  static auto sm();
};
template <class S> decltype(auto) h() { return S(); }
template <class S> __auto_type u() { return S(); }
template <class S> void g() {
  auto a = [](auto x) { return x; };
  auto b = [](S x) -> auto { return x; };
  auto c = [](S x) -> __auto_type { return x; };
  auto d = [](S x) -> decltype(auto) { return x; };
  auto ok = [](S x) { return x; };
  auto nested = [](S x) { return [x] { return x; }(); };
}
void use() { g<int>(); }
