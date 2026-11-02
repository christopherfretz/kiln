// [over.ics.list]/8, [dcl.init.aggr]/16: brace elision does not reach into
// a non-aggregate class member (it takes exactly one clause), and an empty
// class member takes at most one, so longer lists convert to no aggregate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:3 error "no matching function for call to 'f1'"
//@ diag: 14:6 note "candidate function not viable: cannot convert initializer list argument to 'D1'"
//@ diag: 18:3 error "no matching function for call to 'f2'"
//@ diag: 15:6 note "candidate function not viable: cannot convert initializer list argument to 'D2'"
struct E {};
struct C { constexpr C() {} constexpr C(int) {} int a = 0, b = 0; };
struct D1 { C c; int y; };
struct D2 { int i; E x; int c; };
void f1(D1);
void f2(D2);
void test() {
  f1({1, 2, 3});
  f2({1, {}, {}, 2});
}
