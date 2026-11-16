// [over.match.copy]/1, [expr.call]/7, [dcl.struct.bind]/4: a function
// argument and an array structured binding's elements are
// copy-initialized, so an explicit copy constructor is not a candidate.
// The binding's error is at the identifier list and names the array type;
// the argument's carries the parameter note. Found running GCC's C++
// testsuite. Matches clang 22.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 25:8 error "no matching constructor for initialization of 'E[1]'"
//@ diag: 19:12 note "explicit constructor is not a candidate"
//@ diag: 18:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 26:8 error "no matching constructor for initialization of 'E'"
//@ diag: 19:12 note "explicit constructor is not a candidate"
//@ diag: 18:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 23:12 note "passing argument to parameter here"
struct E {
  E() {}
  explicit E(const E&) {}
};
E ge;
E arr[1];
void take(E);
void f() {
  auto [x] = arr;
  take(ge);
}
