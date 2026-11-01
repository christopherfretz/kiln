// [stmt.return]/2: a returned value is converted to the return type as
// written, so the diagnostics name its typedef (clang prints the
// sugared type, with "aka" or the vector suffix) (WP-173).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 17:17 error "no viable conversion from returned value of type 'X' to function return type 'I' (aka 'int')"
//@ diag: 18:19 error "no viable conversion from returned value of type 'X' to function return type 'g4i' (vector of 4 'int' values)"
//@ diag: 19:19 error "cannot initialize return object of type 'g4i' (vector of 4 'int' values) with an rvalue of type 'double'"
//@ diag: 20:17 error "cannot initialize return object of type 'P' (aka 'int *') with an rvalue of type 'int'"
//@ diag: 21:23 error "no viable conversion from returned value of type 'X' to function return type 'const I' (aka 'const int')"
//@ diag: 22:27 error "no viable conversion from returned value of type 'X' to function return type 'I' (aka 'int')"
typedef int g4i __attribute__((vector_size(16)));
typedef int I;
using P = int *;
struct X {};
I r0() { return X{}; }
g4i r1() { return X{}; }
g4i r2() { return 1.5; }
P r3() { return 1; }
const I r4() { return X{}; }
struct S { I m() { return X{}; } };
