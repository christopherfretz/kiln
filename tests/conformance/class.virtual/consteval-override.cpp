// [class.virtual]/19: a consteval function shall not override a non-consteval
// one and vice versa.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(kiln): 'consteval' member functions are not supported yet; the override check is in place
//@ diag: 15:17 error "non-consteval function 'c' cannot override a consteval function"
//@ diag: 11:25 note "overridden virtual function is here"
//@ diag: 16:17 error "consteval function 'n' cannot override a non-consteval function"
//@ diag: 12:25 note "overridden virtual function is here"
struct A {
  virtual consteval int c() { return 1; }
  virtual constexpr int n() { return 2; }
};
struct B : A {
  constexpr int c() override { return 3; }
  consteval int n() override { return 4; }
};
