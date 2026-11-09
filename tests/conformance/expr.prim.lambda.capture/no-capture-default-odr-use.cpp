// [expr.prim.lambda.capture]/8, [basic.def.odr]/5: a lambda with no
// capture-default may name a constexpr local without odr-using it
// (`ca.v`, `ca.arr[1]`, `carr[1]`, `ca.*pm`), but an odr-use of it (its
// address, a reference binding, a read of a variable with a mutable
// member) needs a capture: clang's
// `err_lambda_impcap` and its fix-it notes (WP-1144).
//@ verify
struct A { int v; int arr[2]; };
struct M { int v; mutable int m; };
int use(const int &);

int f() {
  constexpr A ca{4, {5, 6}}; // expected-note 2 {{'ca' declared here}}
  constexpr int carr[2] = {7, 8};
  constexpr M cm{1, 2}; // expected-note {{'cm' declared here}}
  constexpr int A::*pm = &A::v;
  auto ok = [] { return ca.v + ca.arr[1] + carr[1] + ca.*pm; };
  auto addr = [] { return *&ca.v; }; // expected-error {{variable 'ca' cannot be implicitly captured in a lambda with no capture-default specified}} \
    expected-note {{lambda expression begins here}} expected-note {{capture 'ca' by value}} \
    expected-note {{capture 'ca' by reference}} expected-note {{default capture by value}} \
    expected-note {{default capture by reference}}
  auto ref = [] { return use(ca.v); }; // expected-error {{variable 'ca' cannot be implicitly captured in a lambda with no capture-default specified}} \
    expected-note {{lambda expression begins here}} expected-note {{capture 'ca' by value}} \
    expected-note {{capture 'ca' by reference}} expected-note {{default capture by value}} \
    expected-note {{default capture by reference}}
  auto mut = [] { return cm.v; }; // expected-error {{variable 'cm' cannot be implicitly captured in a lambda with no capture-default specified}} \
    expected-note {{lambda expression begins here}} expected-note {{capture 'cm' by value}} \
    expected-note {{capture 'cm' by reference}} expected-note {{default capture by value}} \
    expected-note {{default capture by reference}}
  return ok() + addr() + ref() + mut();
}
