// [dcl.fct]/3, [depr.ellipsis.comma] (C++26, P3176R1): a C variadic
// ellipsis that follows a parameter without a comma is deprecated; an
// ellipsis that is the only parameter-declaration, one after a comma,
// and a function parameter pack (`T...` naming a pack) are not. Also for
// function types, lambdas and templates. After a pack's own `...`
// (`Ts......`) it is -Wambiguous-ellipsis too.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
//@ requires: clang>=22
//@ diag: 20:11 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 23:17 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 24:22 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 25:16 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 26:28 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 29:21 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 31:38 warning "declaration of a variadic function without a comma before '...' is deprecated [-Wdeprecated-missing-comma-variadic-parameter]"
//@ diag: 31:38 warning "'...' in this location creates a C-style varargs function [-Wambiguous-ellipsis]"
//@ diag: 31:35 note "preceding '...' declares a function parameter pack"
//@ diag: 31:38 note "insert ',' before '...' to silence this warning"
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
template <class... Ts> void amb(Ts......);
