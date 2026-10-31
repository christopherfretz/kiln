// [expr.prim.lambda.capture]/7, /8: a local entity odr-used in a lambda
// without a capture-default, and not captured explicitly, is an error
// (with a note at the declaration and fix-it notes at the introducer).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:30 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 44:13 note "'a' declared here"
//@ diag: 44:18 note "lambda expression begins here"
//@ diag: 44:19 note "capture 'a' by value"
//@ diag: 44:19 note "capture 'a' by reference"
//@ diag: 44:19 note "default capture by value"
//@ diag: 44:19 note "default capture by reference"
//@ diag: 45:38 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 45:13 note "'a' declared here"
//@ diag: 45:25 note "lambda expression begins here"
//@ diag: 45:27 note "capture 'a' by value"
//@ diag: 45:27 note "capture 'a' by reference"
//@ diag: 46:39 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 46:13 note "'a' declared here"
//@ diag: 46:25 note "lambda expression begins here"
//@ diag: 46:28 note "capture 'a' by value"
//@ diag: 46:28 note "capture 'a' by reference"
//@ diag: 47:35 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 47:13 note "'a' declared here"
//@ diag: 47:18 note "lambda expression begins here"
//@ diag: 47:24 note "capture 'a' by value"
//@ diag: 47:24 note "capture 'a' by reference"
//@ diag: 47:19 note "default capture by value"
//@ diag: 47:19 note "default capture by reference"
//@ diag: 48:41 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 48:13 note "'a' declared here"
//@ diag: 48:18 note "lambda expression begins here"
//@ diag: 48:30 note "capture 'a' by value"
//@ diag: 48:30 note "capture 'a' by reference"
//@ diag: 48:19 note "default capture by value"
//@ diag: 48:19 note "default capture by reference"
//@ diag: 49:51 error "variable 'a' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 49:30 note "'a' declared here"
//@ diag: 49:35 note "lambda expression begins here"
//@ diag: 49:40 note "capture 'a' by value"
//@ diag: 49:40 note "capture 'a' by reference"
//@ diag: 49:36 note "default capture by value"
//@ diag: 49:36 note "default capture by reference"
void f1(int a) { [] { return a; }(); }
void f3(int a, int b) { [b] { return a; }(); }
void f4(int a, int b) { [&b] { return a; }(); }
void f5(int a) { [x = 1] { return a; }(); }
void f6(int a) { [ x = 1 + 2 ] { return a; }(); }
struct U { int m; void k(int a) { [this] { return a; }(); } };
