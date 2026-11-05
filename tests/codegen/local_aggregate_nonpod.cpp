// [dcl.init.general]/7, [expr.const]/7: an automatic aggregate is stored
// as a constant only when it is POD (clang `IsConstantAggregate` in
// `EmitAutoVarAlloca`, spec/codegen.md "Static initialization"); a non-POD
// one is constructed as written, whatever its initializer folds to, and
// only a `const` one's value reaches its constant destruction. The fold
// prepass does not evaluate the others (WP-633): a class holding a large
// class by value would be built once per variable.
struct Big { int v[4]; long w; };
struct Pod { Big b; int n; };
struct Holder { Big b{{1, 2, 3, 4}, 5}; int n = 1; ~Holder(); };
struct Wrap { Holder h; Big extra[2]{}; };
struct CD { int x; constexpr ~CD() { if (x) {} } };
void use(const void*);

void f() {
  Pod p = {{{1, 2, 3, 4}, 5}, 2};   // POD: a constant
  Holder h;                         // non-POD: constructed as written
  Holder hs[2];
  Wrap w = {};
  const CD c{1};                    // const: constant destruction
  CD d{1};                          // not const: its destructor runs
  use(&p); use(&h); use(hs); use(&w); use(&c); use(&d);
}
