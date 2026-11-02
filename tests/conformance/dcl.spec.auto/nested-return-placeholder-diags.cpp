// [dcl.spec.auto.general]/3-4, [dcl.type.auto.deduct]/2-4, [dcl.fct]/22: a
// placeholder in a nested function return type. A variable needs an
// initializer; deduction needs matching parameters and exception
// specification; a type-constraint is checked against the deduced type
// (reported at the constraint, also for an invented template parameter);
// `decltype(auto)` is never a parameter's placeholder, and `auto` invents
// a template parameter only in a function declaration's own clause.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:8 error "declaration of variable 'v1' with deduced type 'auto (*)(int) -> auto (*)(int) -> auto' requires an initializer"
//@ diag: 25:8 error "variable 'v2' with type 'auto (*)(long) -> auto' has incompatible initializer of type 'int (int)'"
//@ diag: 26:8 error "variable 'v3' with type 'auto (*)(int) noexcept -> auto' has incompatible initializer of type 'int (int)'"
//@ diag: 27:37 error "deduced type 'long' does not satisfy 'C'"
//@ diag: 21:32 note "because 'sizeof(long) == 4' (8 == 4) evaluated to false"
//@ diag: 30:9 error "no matching function for call to 'f'"
//@ diag: 28:6 note "candidate template ignored: constraints not satisfied [with cb:auto = long]"
//@ diag: 28:44 note "because 'long' does not satisfy 'C'"
//@ diag: 21:32 note "because 'sizeof(long) == 4' (8 == 4) evaluated to false"
//@ diag: 32:9 error "'decltype(auto)' not allowed in function prototype"
//@ diag: 33:18 error "'auto' not allowed in function prototype that is not a function declaration"
template <class T> concept C = sizeof(T) == 4;
int inc(int);
long (*getl(int))(int);
auto (*v1)(int) -> auto (*)(int) -> auto;
auto (*v2)(long) -> auto = inc;
auto (*v3)(int) noexcept -> auto = inc;
auto (*v4)(int) -> auto (*)(int) -> C auto = getl;
auto f(auto (*cb)(int) -> auto (*)(int) -> C auto) { return cb; }
void use() {
  (void)f(getl);
}
void g1(auto (*cb)(int) -> auto (*)(int) -> decltype(auto));
void g2(int (*q)(auto (*cb)(int) -> auto (*)(int) -> auto));
