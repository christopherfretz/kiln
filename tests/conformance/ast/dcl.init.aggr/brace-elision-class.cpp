//@ expect: ast
//@ ast-filter: use
// [dcl.init.aggr]/16: brace elision applies only to subaggregates; an
// element of a class with a user-declared constructor is
// copy-initialized from its initializer-clause.
struct G { G(int); int v; };
struct A { int x, y; };
void use() {
  G g[2]{1, 2};
  A a[2]{1, 2, 3};
  G* p = new G[2]{3, 4};
}
