// [temp.inst], [expr.const]: an error in a template's definition is
// diagnosed once, at the definition. Instantiating the template does not
// fail with an internal error and adds no follow-on diagnostic: the
// erroneous expression substitutes to an erroneous expression, and a
// constant evaluation that reaches it fails without a note (clang's
// RecoveryExpr / containsErrors behavior; WP-141).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:44 error "use of undeclared identifier 'nosuch'"
//@ diag: 20:51 error "use of undeclared identifier 'nosuch2'"
//@ diag: 21:60 error "use of undeclared identifier 'nosuch3'"
//@ diag: 22:50 error "use of undeclared identifier 'nosuch4'"
//@ diag: 24:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 26:15 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 22:63 note "control reached end of constexpr function"
//@ diag: 26:19 note "in call to 'h<int>(1)'"
//@ requires: clang>=22
// clang 22 spelling of recovery-expr-in-pattern.cpp (WP-394).
template <class T> int f(T t) { return t + nosuch; }
template <class T> constexpr int g(T t) { int x = nosuch2(1); return x + t; }
template <class T> struct S { int m() { return sizeof(T) + nosuch3; } };
template <class T> constexpr int h(T t) { return nosuch4 + t; }
int a = f(1);
constexpr int c = g(1);
int b = S<int>().m();
constexpr int d = h(1);
