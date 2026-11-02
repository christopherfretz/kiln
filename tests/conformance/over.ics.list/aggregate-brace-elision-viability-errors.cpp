// [over.ics.list]/8, [dcl.init.aggr]/16: `{{}, {}, 1}` initializes `B::e`
// from the first `{}` (braces are not elided for a braced clause) and
// `B::x` from the second, so `1` is an excess clause and `B` is not a
// viable parameter type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:3 error "no matching function for call to 'm'"
//@ diag: 12:6 note "candidate function not viable: cannot convert initializer list argument to 'B'"
//@ diag: 13:6 note "candidate function not viable: cannot convert initializer list argument to 'int'"
struct E {};
struct B { E e[2]; int x; };
void m(B);
void m(int);
void test() {
  m({{}, {}, 1});
}
