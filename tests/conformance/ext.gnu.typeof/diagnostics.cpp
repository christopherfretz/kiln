// GNU typeof (spec/sema.md §5.8) diagnostics: the operand is checked as
// any unevaluated operand (an unresolved overload set, an undeclared name,
// an empty operand), an erroneous typeof combines with no other type
// specifier, and the type prints as written ('typeof (i)', 'typeof(int)').
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:9 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 17:19 note "possible target for call"
//@ diag: 17:6 note "possible target for call"
//@ diag: 19:10 error "use of undeclared identifier 'unknown'"
//@ diag: 20:10 error "expected expression"
//@ diag: 21:14 error "use of undeclared identifier 'missing'"
//@ diag: 23:14 error "cannot initialize a variable of type 'typeof (i) *' (aka 'int *') with an rvalue of type 'double'"
//@ diag: 24:16 error "cannot initialize a variable of type 'typeof(int) *' (aka 'int *') with an lvalue of type 'const char[2]'"
//@ diag: 26:17 error "no viable conversion from 'S' to 'typeof (S().a)' (aka 'int')"
//@ diag: 28:24 error "cannot initialize a variable of type 'typeof_unqual (ci) *' (aka 'int *') with an rvalue of type 'const int *'"
void g(int); void g(double);
__typeof(g) *pg;
__typeof(unknown) u;
__typeof() e;
int __typeof(missing) e2;
int i;
__typeof(i) *p = 1.5;
__typeof(int) *q = "x";
struct S { int a; };
__typeof(S().a) z = S();
const int ci = 0;
__typeof_unqual__(ci) *r = &ci;
