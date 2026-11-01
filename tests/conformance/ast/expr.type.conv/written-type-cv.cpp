//@ expect: ast
//@ oracle: clang
// [expr.type.conv]/1-2, [expr.static.cast]: a named or functional cast
// prints its type as written (`CI`, `R`), although a non-class prvalue
// drops the qualifiers ([expr.type]/2) and the alias; a class temporary
// keeps the written type, qualifiers and sugar included: `CV(1)` and
// `CV(1, 2)` construct a `'CV':'const V'`, as `CV{2}` does.
using CI = const int;
using R = int&;
struct V { int v; V(int x) : v(x) {} V(int x, int y) : v(x + y) {} };
using CV = const V;
struct A { int a, b; };
using CA = const A;
void f(int x) {
  (void)CI{1};
  (void)CI(2);
  (void)static_cast<CI>(3);
  (void)static_cast<R>(x);
  (void)CV(1);
  (void)CV(1, 2);
  (void)CV{2};
  (void)CA(1, 2);
}
