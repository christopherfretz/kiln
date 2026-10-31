// Local and unnamed class IR names (clang printQualifiedName) and the
// internal linkage of local classes and unnamed-type globals.
namespace n { struct O { int f() { struct L { struct M { int q; } m; int z; }; L::M mm{}; L l{}; auto lam = []{ struct K { int a; } k{}; return k.a; }; return l.z + mm.q + lam(); } }; }
int u() { return n::O().f(); }
struct { struct N { int a; } n; } v;
union { struct N2 { int a; } n; int z; } w;
int f(int, ...);
int f(int x, ...) { struct Q { int a; struct R { int b; } r; }; Q::R r{}; return r.b; }
void* u2() { return &v.n + (long)&w; }
namespace q { int f3(const char* s, int (&a)[3]) { return [](long w){ struct K { struct J { int a; } j; }; K::J j{}; return j.a; }(1); } }
int u3() { int a[3]{}; return q::f3("", a); }
