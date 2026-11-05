// [dcl.init.list]/7.5, [conv.bool] (C++20 DR, P1957R2): a conversion from
// a pointer or pointer-to-member type to bool is narrowing.
// WP-646 feature probe.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 13:10 error "type 'void *' cannot be narrowed to 'bool' in initializer list"
//@ diag: 13:10 note "insert an explicit cast to silence this issue"
//@ diag: 14:10 error "type 'int S::*' cannot be narrowed to 'bool' in initializer list"
//@ diag: 14:10 note "insert an explicit cast to silence this issue"
struct S { int m; };
void f(void* p, int S::*pm) {
  bool a{p};
  bool b{pm};
  bool c = p;
}
