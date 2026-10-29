// typeid before std::type_info is declared ([expr.typeid]/7): clang's
// err_need_header_before_typeid. A namespace std without type_info is
// not enough.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:9 error "you need to include <typeinfo> before using the 'typeid' operator"
//@ diag: 11:9 error "you need to include <typeinfo> before using the 'typeid' operator"
namespace std { struct other; }
void f(int x) {
  (void)typeid(int);
  (void)typeid(x);
}
