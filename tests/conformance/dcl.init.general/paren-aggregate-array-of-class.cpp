// [dcl.init.general]/16.6.2.2, [dcl.init.aggr]/5: `B(1)` initializes the
// aggregate `B` from the parenthesized list; the array-of-class member
// without an argument is value-initialized by `S`'s default constructor,
// in a template as outside one (WP-307 d4, WP-316). clang 19 rejects the
// instantiated `return B(1);` ("array initializer must be an initializer
// list"); clang 22 and clang 19's non-template path accept it
// (docs/VALIDATION.md, known oracle defects).
//@ requires: clang>=22
struct S { S(); S(int, int = 1); };
struct B { int i; S arr[2]; };
void take(B);
template <class T> void f(T t) { take(B{1}); take({1}); B b = {1}; (void)b; B c{}; (void)B{}; (void)B(); }
template void f(int);
template <class T> B g(T) { return {}; }
B x = g(1);
template <class T> B h(T) { return B(1); }
B y = h(1);
struct C { S arr[2]; int j; };
B z = B(1, {S(), S(1, 2)});
C w = C({S(1, 3)});
template <class T> C cc(T t) { return C({S(t, t)}, t); }
C v = cc(4);
