// [expr.prim.lambda.capture]/6 (P2036R3): an init-capture named in a
// generic lambda's requires-clause; a call whose requirement on it fails
// is "constraints not satisfied" (WP-1497).
//@ expect: compile-fail
//@ std: c++23
//@ diag: 13:9 error "no matching function for call to object of type"
//@ diag: 10:10 note "candidate template ignored: constraints not satisfied [with a:auto = int]"
//@ diag: 10:47 note "because 'f(a)' would be invalid: no matching function for call to object of type"
template <class Fn> constexpr auto generic(Fn fn) {
  return [f = fn](auto a) requires requires { f(a); } { return f(a); };
}
struct NoInt {};
int x = generic([](NoInt) { return 0; })(1);
