// GNU `__extension__` keeps its operand's semantics for diagnostics too
// (WP-1029): `__extension__ 0` is an int rvalue, not a null pointer
// constant ([conv.ptr]/1), the address of an `__extension__` bit-field
// is ill-formed ([expr.unary.op]/3, [class.bit]/3), and an overload set
// under `__extension__` still needs a target ([over.over]/1). Verified
// against clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:6 error "cannot initialize a variable of type 'int *' with an rvalue of type 'int'"
//@ diag: 17:11 error "address of bit-field requested"
//@ diag: 20:15 error "reference to overloaded function could not be resolved; did you mean to call it?"
//@ diag: 19:6 note "possible target for call"
//@ diag: 18:6 note "possible target for call"
struct B { int bf : 3; };
int *np = __extension__ 0;
B b;
int *bp = &__extension__ b.bf;
void f(int);
void f(double);
int x = sizeof(__extension__ f);
