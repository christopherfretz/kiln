// [dcl.init]/17.6.3, [conv.mem]: a pointer to member initialized from one of a
// different class, member type or function type. The function-type
// mismatches are explained as clang does (HandleFunctionTypeMismatch).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:11 error "cannot initialize a variable of type 'void (C::*)(int)' with an rvalue of type 'void (C::*)(int) const': different qualifiers (unqualified vs 'const')"
//@ diag: 19:11 error "cannot initialize a variable of type 'void (C::*)()' with an rvalue of type 'void (C::*)(int) const': different number of parameters (0 vs 1)"
//@ diag: 20:10 error "cannot initialize a variable of type 'int (C::*)()' with an rvalue of type 'void (C::*)(int) const': different number of parameters (0 vs 1)"
//@ diag: 21:10 error "cannot initialize a variable of type 'long C::*' with an rvalue of type 'int C::*'"
//@ diag: 22:9 error "cannot initialize a variable of type 'int D::*' with an rvalue of type 'int C::*': different classes ('D' vs 'C')"
//@ diag: 23:9 error "cannot initialize a variable of type 'int C::*' with an rvalue of type 'int'"
//@ diag: 24:8 error "cannot initialize a variable of type 'void (*)()' with an lvalue of type 'void (int)': different number of parameters (0 vs 1)"
//@ diag: 25:8 error "cannot initialize a variable of type 'void (*)(int)' with an rvalue of type 'int (*)(long)': type mismatch at 1st parameter ('int' vs 'long')"
struct C { int m; void f(int) const; };
struct D { int m; };
void fn(int);
int gn(long);
void (C::*a)(int) = &C::f;
void (C::*b)() = &C::f;
int (C::*c)() = &C::f;
long C::*d = &C::m;
int D::*e = &C::m;
int C::*z = 0 + 1;
void (*p)() = fn;
void (*q)(int) = &gn;
