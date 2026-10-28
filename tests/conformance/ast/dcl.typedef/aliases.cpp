//@ expect: ast
typedef int Int;
typedef Int Int2;
using Ptr = const Int *;
using Fn = int(int, Ptr);
typedef struct { int v; } Anon;
Int2 twice(Int2 x) { return x * 2; }
int deref(Ptr p) { return *p; }
int call(Fn *f, Int a) { return f(a, &a); }
int anon(Anon a) { return a.v; }
