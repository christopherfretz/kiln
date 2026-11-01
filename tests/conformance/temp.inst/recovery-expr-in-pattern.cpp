// [temp.inst], [expr.const]: an error in a template's definition is
// diagnosed once, at the definition. Instantiating the template does not
// fail with an internal error and adds no follow-on diagnostic: the
// erroneous expression substitutes to an erroneous expression, and a
// constant evaluation that reaches it fails without a note (clang's
// RecoveryExpr / containsErrors behavior; WP-141).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:44 error "use of undeclared identifier 'nosuch'"
//@ diag: 16:51 error "use of undeclared identifier 'nosuch2'"
//@ diag: 17:60 error "use of undeclared identifier 'nosuch3'"
//@ diag: 18:50 error "use of undeclared identifier 'nosuch4'"
//@ diag: 20:15 error "must be initialized by a constant expression"
//@ diag: 22:15 error "must be initialized by a constant expression"
template <class T> int f(T t) { return t + nosuch; }
template <class T> constexpr int g(T t) { int x = nosuch2(1); return x + t; }
template <class T> struct S { int m() { return sizeof(T) + nosuch3; } };
template <class T> constexpr int h(T t) { return nosuch4 + t; }
int a = f(1);
constexpr int c = g(1);
int b = S<int>().m();
constexpr int d = h(1);
