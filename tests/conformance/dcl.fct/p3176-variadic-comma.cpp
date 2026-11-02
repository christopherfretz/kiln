// [dcl.fct]/3, [depr.ellipsis.comma] (C++26, P3176R1): a C variadic
// ellipsis that follows a parameter without a comma is deprecated; an
// ellipsis that is the only parameter-declaration, one after a comma,
// and a function parameter pack (`T...` naming a pack) are not. Also for
// function types, lambdas and templates.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
//@ requires: clang>=22
//@ diag: 16:11 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 19:17 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 20:22 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 21:16 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 22:28 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 25:21 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
void f(int...);
void g(int, ...);
void h(...);
void named(int n...);
using F = void(double...);
auto l = [](int...) {};
template <class T> void t(T...);
template <class... Ts> void pack(Ts...);
struct S {
  void m(const char*...);
};
