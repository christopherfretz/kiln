// [expr.prim.lambda.capture]/6 (P2036R3): an init-capture named in a
// generic lambda's requires-clause; a call whose requirement on it fails
// is "constraints not satisfied" (WP-1497); the init-capture is named as
// its variable, without the call operator's const (WP-1509).
//@ expect: compile-fail
//@ std: c++23
//@ diag: 14:9 error "no matching function for call to object of type"
//@ diag: 11:10 note "candidate template ignored: constraints not satisfied [with a:auto = int]"
//@ diag: 11:47 note "because 'f(a)' would be invalid: no matching function for call to object of type '(lambda at"
template <class Fn> constexpr auto generic(Fn fn) {
  return [f = fn](auto a) requires requires { f(a); } { return f(a); };
}
struct NoInt {};
int x = generic([](NoInt) { return 0; })(1);
