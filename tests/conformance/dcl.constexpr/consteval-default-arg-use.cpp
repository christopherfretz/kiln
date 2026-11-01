// [expr.const]/16 (WP-137): the immediate invocations of a default
// argument are evaluated where it is used, and one that is not a constant
// expression is reported there ("in the default initializer of"). A
// default argument used in another one is checked where that is defined;
// a non-dependent use in a template, at the definition and again at each
// instantiation (clang rebuilds it).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:15 error "call to consteval function 'h' is not a constant expression"
//@ diag: 53:21 note "in the default initializer of 'p'"
//@ diag: 48:7 note "declared here"
//@ diag: 48:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
//@ diag: 48:15 error "call to consteval function 'h' is not a constant expression"
//@ diag: 54:31 note "in the default initializer of 'p'"
//@ diag: 48:7 note "declared here"
//@ diag: 48:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
//@ diag: 50:27 error "call to consteval function 'h' is not a constant expression"
//@ diag: 55:17 note "in the default initializer of 'q'"
//@ diag: 50:19 note "declared here"
//@ diag: 50:29 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
//@ diag: 50:27 error "call to consteval function 'h' is not a constant expression"
//@ diag: 55:23 note "in the default initializer of 'q'"
//@ diag: 50:19 note "declared here"
//@ diag: 50:29 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
//@ diag: 48:15 error "call to consteval function 'h' is not a constant expression"
//@ diag: 51:20 note "in the default initializer of 'p'"
//@ diag: 48:7 note "declared here"
//@ diag: 48:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
//@ diag: 48:15 error "call to consteval function 'h' is not a constant expression"
//@ diag: 57:38 note "in the default initializer of 'p'"
//@ diag: 48:7 note "declared here"
//@ diag: 48:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
//@ diag: 48:15 error "call to consteval function 'h' is not a constant expression"
//@ diag: 58:21 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 57:38 note "in the default initializer of 'p'"
//@ diag: 48:7 note "declared here"
//@ diag: 48:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:5 note "declared here"
consteval int h(int x) { return x; }
int k;
constexpr int c = 4;
int f(int p = h(k));
int g(int p = h(c)) { return p; }
struct X { X(int, int q = h(k)) {} };
int nested(int r = f());
int use1() { return f(2) + g() + g(1); }
int use2() { return f(); }
constexpr int use3() { return f(); }
void use4() { X x(1); X{1}; }
int use5() { return nested(); }
template <class T> int tf() { return f(); }
int use6() { return tf<int>(); }
