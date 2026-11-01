// [dcl.init.ref]/5: the failures of reference binding, with clang's
// wording. A non-const lvalue reference binds no rvalue, an xvalue
// included (/5.2); otherwise a reference to an unrelated or incomplete
// type "could not bind" (an xvalue reads "rvalue"); a function lvalue
// binds no rvalue reference to another function type; a braced
// temporary of a different type is not bound to a non-const lvalue
// reference (/5.3, /5.4).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:6 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 38:15 error "volatile lvalue reference to type 'volatile int' cannot bind to a temporary of type 'int'"
//@ diag: 39:9 error "reference to type 'void ()' could not bind to an lvalue of type 'int (int)'"
//@ diag: 40:21 error "volatile lvalue reference to type 'const volatile int' cannot bind to a temporary of type 'int'"
//@ diag: 41:21 error "volatile lvalue reference to type 'const volatile int' cannot bind to a temporary of type 'int'"
//@ diag: 42:7 error "non-const lvalue reference to type 'int[2]' cannot bind to a temporary of type 'int[2]'"
//@ diag: 43:10 error "reference to type 'const T' could not bind to an rvalue of type 'int'"
//@ diag: 45:12 error "excess elements in scalar initializer"
//@ diag: 46:7 error "rvalue reference to type 'int' cannot bind to lvalue of type 'int'"
//@ diag: 47:23 error "type 'double' cannot be narrowed to 'int' in initializer list [-Wc++11-narrowing]"
//@ diag: 47:23 note "insert an explicit cast to silence this issue"
//@ diag: 47:23 warning "implicit conversion from 'double' to 'int' changes value from 1.5 to 1 [-Wliteral-conversion]"
//@ diag: 50:4 error "non-const lvalue reference to type 'B' cannot bind to a temporary of type 'const B'"
//@ diag: 51:8 error "non-const lvalue reference to type 'void ()' cannot bind to a value of unrelated type 'int (int)'"
//@ diag: 53:14 warning "binding dereferenced null pointer to reference has undefined behavior [-Wnull-dereference]"
//@ diag: 53:6 error "non-const lvalue reference to type 'int' cannot bind to a temporary of type 'int'"
//@ diag: 54:18 warning "binding dereferenced null pointer to reference has undefined behavior [-Wnull-dereference]"
//@ diag: 54:7 error "non-const lvalue reference to type 'int[]' cannot bind to a temporary of type 'int[2]'"
//@ diag: 55:14 error "reference to incomplete type 'const long[]' could not bind to an rvalue of type 'int[2]'"
//@ diag: 56:14 error "reference to incomplete type 'const long[]' could not bind to an lvalue of type 'int[2]'"
//@ diag: 58:7 error "reference to incomplete type 'Inc' could not bind to an rvalue of type 'int'"
//@ diag: 59:12 error "reference to incomplete type 'const Inc' could not bind to an lvalue of type 'int'"
//@ diag: 60:7 error "reference to incomplete type 'Inc' could not bind to an lvalue of type 'int'"
template<class X> X&& mv(X& x) { return static_cast<X&&>(x); }
int i; void g(); int h(int);
struct S { operator int&&(); operator int(); };
struct T { int m; };
int& r1 = static_cast<int&&>(i);
volatile int& r2 = mv(i);
void (&&r4)() = h;
const volatile int& r5 = 5;
const volatile int& r6 = mv(i);
int (&r7)[2] = static_cast<int(&&)[2]>(*(int(*)[2])nullptr);
const T& r8 = 5;
T&& r9 = {};
const int& r10 = {1, 2};
int&& r11 = {i};
const int (&r12)[] = {1.5};
void g2();
struct B { int bf : 3; }; const B cs{};
B& q1 = static_cast<const B&&>(*(B*)0);
void (&q2)() = h;
const B& q3 = mv(cs);
int& q4 = mv(*(B*)0).bf;
int (&q5)[] = mv(*(int(*)[2])0);
const long (&q6)[] = (int(&&)[2])*(int(*)[2])0;
const long (&q7)[] = *(int(*)[2])0;
struct Inc;
Inc&& x1 = static_cast<int&&>(i);
const Inc& x2 = i;
Inc&& x3 = i;
