//@ expect: ast
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ oracle: none
// [dcl.attr.annotation]/3 and [meta.reflection.annotation]: every
// declaration keeps the annotations written on it, and an entity's
// annotations are those of all its declarations in declaration order (no
// merging of equal values). A function template specialization takes the
// annotations of every declaration of its template.
[[=1]] void f();
[[=2]] void f();
[[=3]] void f() { }
struct [[=4]] S;
struct [[=5]] S { };
namespace [[=6]] n { }
namespace [[=7]] n { }
[[=8]] extern int v;
[[=9]] int v = 0;
template <int N> [[=N]] int g();
template <int N> [[=N + 10]] int g() { return N; }
int use = g<3>();
