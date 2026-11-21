// [basic.link]/15-17 (WP-1684): what stays valid next to the forms
// exposure-member-init-default-arg.cpp and exposure-odr-use-and-closure.cpp
// reject. A constant read in a default member initializer is not an
// odr-use (/14.4; a TU-local type in a sizeof operand is named:
// exposure-unevaluated-discarded.cpp, WP-1821); a member function
// defined in its class in a named module is not implicitly inline, so its
// body is ignored (/15.1.1), odr-uses included; a function template's body
// is checked only when instantiated; a closure type whose call operator
// names no TU-local entity, or only reads a constant, exposes nothing.
// GCC 16.1 and clang 22 accept all of it.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags

export module M;
static int helper() { return 1; }
static const int K = 5;
static int hidden;
namespace {
struct S { int v; };
}

export struct ReadsConstant { int m = K; };
export inline int read_k() { return K; }
export struct NotInline {
  const int *address() { return &K; }
  static const int *static_address() { return &K; }
  const int &reference() { return K; }
  int call() { return helper(); }
  int *variable() { return &hidden; }
};
struct ModuleLinkage { const int *f() { return &K; } };
export inline const int *calls_member() { return NotInline().address(); }
export template <class T> const int *function_template(T) { return &K; }
export inline auto reads_in_closure = [] { return K; };
export inline auto plain_closure = [](int x) { return x; };
