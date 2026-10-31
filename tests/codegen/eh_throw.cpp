// [expr.throw]: throw-expressions (__cxa_allocate_exception, the
// thrown object's construction with __cxa_free_exception on unwind,
// __cxa_throw / __cxa_rethrow), in conditionals and under cleanups.
struct S { int a; S(int); S(const S&); ~S(); };
struct T { T(); ~T(); };
struct P { int x, y; };
int g(int);
void f1() { throw 1; }
void f2(int x) { throw x + 1; }
void f3() { throw S(3); }
void f4(S& s) { throw s; }
void f5() { throw P{1, 2}; }
void f6() { throw; }
int f7(int c) { return c ? throw 2 : g(c); }
void f8() { T t; throw S(g(1)); }
void f9(int c) { int x = 0; (c ? x : throw 1) = 3; }
void f10(int c) { if (c ? throw 1 : g(c)) g(2); }
void f11() { throw "hi"; }
void f12(int *p) { throw p; }
void f13(int c) { c ? throw S(1) : throw S(2); }
