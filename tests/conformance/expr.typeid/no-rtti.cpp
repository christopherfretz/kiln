// -fno-rtti: clang's err_no_typeid_with_fno_rtti (after the
// <typeinfo> check).
//@ expect: compile-fail
//@ args: -fno-rtti
//@ strict-diags
//@ diag: 10:9 error "use of typeid requires -frtti"
//@ diag: 11:9 error "use of typeid requires -frtti"
namespace std { class type_info; }
void f(int x) {
  (void)typeid(int);
  (void)typeid(x);
}
