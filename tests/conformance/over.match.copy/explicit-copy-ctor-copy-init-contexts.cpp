// [over.match.copy]/1, [dcl.init.general]/16.6.2, [over.match.ctor]/1: in
// copy-initialization an explicit copy constructor is not a candidate, in
// every context that copy-initializes: `T x = e;`, a return value
// ([dcl.init.general]/15), a thrown object
// ([except.throw]/3), a handler's parameter ([except.handle]/15), a default
// member initializer, and the elements of a parenthesized aggregate
// initialization ([dcl.init.general]/16.6.2.2). The direct forms accept it.
// (Function arguments and array structured bindings: the sibling run test.)
// Found running GCC's C++ testsuite. Matches clang 22; GCC 16.1 accepts the
// handler's parameter.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 39:18 error "no matching constructor for initialization of 'E'"
//@ diag: 34:12 note "explicit constructor is not a candidate"
//@ diag: 33:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 40:19 error "no matching constructor for initialization of 'E'"
//@ diag: 34:12 note "explicit constructor is not a candidate"
//@ diag: 33:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 42:5 error "no matching constructor for initialization of 'E'"
//@ diag: 34:12 note "explicit constructor is not a candidate"
//@ diag: 33:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 43:9 error "no matching constructor for initialization of 'E'"
//@ diag: 34:12 note "explicit constructor is not a candidate"
//@ diag: 33:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 44:9 error "no matching constructor for initialization of 'E'"
//@ diag: 34:12 note "explicit constructor is not a candidate"
//@ diag: 33:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 48:14 error "no matching constructor for initialization of 'E'"
//@ diag: 34:12 note "explicit constructor is not a candidate"
//@ diag: 33:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
struct E {
  E() {}
  explicit E(const E&) {}
};
struct Agg { E a; };
E ge;
E arr[1];
E ret() { return ge; }
struct DM { E m = ge; };
void f() {
  E a = ge;
  throw ge;
  Agg p(ge);
}
void g() {
  try {
  } catch (E c) {
  }
}
// The direct forms select the explicit copy constructor.
void h() {
  E a(ge);
  E b{ge};
  auto [x](arr);
}
