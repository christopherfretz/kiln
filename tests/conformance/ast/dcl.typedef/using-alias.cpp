//@ expect: ast
using Int = int;
using IntPtr = Int *;
using Fn = int (*)(Int);
typedef struct { int x; } Anon;
int apply(Fn f, IntPtr p, Anon a) { return f(*p) + a.x; }
