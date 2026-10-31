// [expr.prim.lambda.closure]/4, [expr.prim.lambda.capture]/12: the call
// operator of a non-mutable lambda is const, so a copy capture (and the
// copy of `*this`) cannot be modified; a reference capture can.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:11 error "cannot assign to a variable captured by copy in a non-mutable lambda"
//@ diag: 17:11 error "cannot assign to a variable captured by copy in a non-mutable lambda"
//@ diag: 20:10 error "cannot assign to a variable captured by copy in a non-mutable lambda"
//@ diag: 22:47 error "cannot assign to non-static data member within const member function 'k'"
//@ diag: 22:24 note "member function 'T::k' is declared const here"
//@ diag: 22:69 error "cannot assign to non-static data member within const member function 'k'"
//@ diag: 22:24 note "member function 'T::k' is declared const here"
//@ diag: 22:102 error "cannot assign to non-static data member within const member function 'k'"
//@ diag: 22:24 note "member function 'T::k' is declared const here"
void f(int a) {
  [a] { a = 1; }();
  [=] { a = 2; }();
  [a] () mutable { a = 1; }();
  [&r = a] { r = 3; }();
  [a] { a++; }();
}
struct T { int m; void k() const { [this] { m = 1; }(); [*this] { m = 2; }(); [*this] () mutable { m = 2; }(); } };
