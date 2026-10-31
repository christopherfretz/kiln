// [expr.prim.lambda.capture]/2, /5, /8: an explicit capture names a local
// entity at most once, not one with static storage duration, and a
// simple-capture repeats the capture-default's kind only for `this`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:7 error "'a' can appear only once in a capture list"
//@ diag: 18:8 error "'&' cannot precede a capture when the capture default is '&'"
//@ diag: 19:7 error "'&' must precede a capture when the capture default is '='"
//@ diag: 20:4 error "'s' cannot be captured because it does not have automatic storage duration"
//@ diag: 16:14 note "'s' declared here"
//@ diag: 21:4 error "'g' cannot be captured because it does not have automatic storage duration"
//@ diag: 14:5 note "'g' declared here"
//@ diag: 22:4 error "use of undeclared identifier 'zz'"
int g;
void f(int a) {
  static int s;
  [a, a] {}();
  [&, &a] {}();
  [=, a] {}();
  [s] {}();
  [g] {}();
  [zz] {}();
  [x = a, x = 2] {}();
}
