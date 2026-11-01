// [intro.execution]/5, [class.temporary]/6.9: a temporary created in a
// default argument is destroyed at the end of the full-expression
// containing the call, after the call; [expr.new]/23, [except.ctor]/3: in
// an array new, the list's partial-array cleanup ends after the first
// element of the filler loop (clang `EmitNewArrayInitializer`).
struct X { X(); ~X(); };
struct T { T(const X & = X()); ~T(); int v; };
struct A { T t; int x; };
void g(const X & = X());
void f1() { g(); }
void f2() { T t; }
void f3() { T t = T(); }
void f4() { T a[3]; }
T *f5() { return new T; }
T *f6(long n) { return new T[n]; }
T *f7(long n) { return new T[n]{X()}; }
A *f8(long n) { return new A[n]{}; }
A *f9(long n) { return new A[n](); }
T *f10(long n) { return new T[n](X(), X()); }
