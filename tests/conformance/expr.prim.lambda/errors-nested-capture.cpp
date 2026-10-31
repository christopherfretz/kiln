// [expr.prim.lambda.capture]/8, [basic.def.odr]/10: an intervening lambda
// that cannot capture an enclosing local makes its odr-use ill-formed, as
// does a local class naming a local of its enclosing function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:46 error "reference to local variable 'a' declared in enclosing function 'f1'"
//@ diag: 37:13 note "'a' declared here"
//@ diag: 38:63 error "reference to local variable 'b' declared in enclosing lambda expression"
//@ diag: 38:28 note "'b' declared here"
//@ diag: 38:18 warning "expression result unused"
//@ diag: 39:54 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 39:13 note "'a' declared here"
//@ diag: 39:42 note "lambda expression begins here"
//@ diag: 39:43 note "capture 'a' by value"
//@ diag: 39:43 note "capture 'a' by reference"
//@ diag: 39:43 note "default capture by value"
//@ diag: 39:43 note "default capture by reference"
//@ diag: 40:55 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 40:13 note "'a' declared here"
//@ diag: 40:42 note "lambda expression begins here"
//@ diag: 40:44 note "capture 'a' by value"
//@ diag: 40:44 note "capture 'a' by reference"
//@ diag: 41:47 error "variable 'x' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 41:19 note "'x' declared here"
//@ diag: 41:35 note "lambda expression begins here"
//@ diag: 41:36 note "capture 'x' by value"
//@ diag: 41:36 note "capture 'x' by reference"
//@ diag: 41:36 note "default capture by value"
//@ diag: 41:36 note "default capture by reference"
//@ diag: 42:52 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 42:13 note "'a' declared here"
//@ diag: 42:40 note "lambda expression begins here"
//@ diag: 42:41 note "capture 'a' by value"
//@ diag: 42:41 note "capture 'a' by reference"
//@ diag: 42:41 note "default capture by value"
//@ diag: 42:41 note "default capture by reference"
void f1(int a) { struct L { int g() { return a; } }; }
void f2(int a) { [=] { int b = a; struct L { int g() { return b; } }; }; }
void f5(int a, int b) { [=, &b] { return [] { return a; }(); }(); }
void f6(int a, int b) { [b, &a] { return [b] { return a; }(); }(); }
void f7(int a) { [x = a] { return [] { return x; }(); }(); }
void f8(int a) { auto b = [&] { return [] { return a; }(); }; }
