// typeid before std::type_info is declared ([expr.typeid]/7): clang's
// err_need_header_before_typeid. A namespace std without type_info is
// not enough.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:9 error "you need to include <typeinfo> before using the 'typeid' operator"
//@ diag: 13:9 error "you need to include <typeinfo> before using the 'typeid' operator"
//@ requires: clang<22
// clang 22 emits error: you need to include <typeinfo> or import std before using the 'typeid' oper...; WP-355 residual
namespace std { struct other; }
void f(int x) {
  (void)typeid(int);
  (void)typeid(x);
}
