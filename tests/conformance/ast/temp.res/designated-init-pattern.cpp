//@ expect: ast
//@ ast-filter: f
// [temp.res], [dcl.init.aggr]/3.1: a dependent designated initializer
// list in a template pattern is an InitListExpr of type void whose items
// are DesignatedInitExprs (field, nested and array designators, a pack
// expansion of a designated clause); a non-dependent one is checked when
// parsed.
struct Pt { int x; int y; int z; };
struct Q { Pt p; int w; };
template<class T> T f1(int v) { return T{.y = v, .x = 2}; }
template<class T> T f2(T v) { return {.p.y = v, .w = 1}; }
template<class T> int f3(T v) { int a[3] = {[1] = v}; return a[1]; }
template<class T, class... U> T f4(U... u) { return T{.x = u...}; }
template<class T> Pt f5(T) { return Pt{.y = 1, .z = 2}; }
