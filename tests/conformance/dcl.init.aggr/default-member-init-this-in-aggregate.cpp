// [dcl.init.aggr]/5.4, [class.mem.general]/9, [expr.prim.this]/2: in an
// aggregate initialization, `this` in a default member initializer is the
// aggregate being initialized, not the enclosing function's object (a
// lambda's closure) nor absent (in a free function); nested aggregates
// each see their own object (a TOML library's `std::function` members
// bound to `[this]`).
//@ expect: run
//@ std: c++20

struct F {
  int v{};
  int w = [this] { return v + 1; }();
  F* self = this;
};

struct Outer {
  int k = 5;
  F f{};
  Outer* me = this;
  int kk = k * 2;
};

struct G {
  int n = 3;
  struct Setter {
    G* g;
    constexpr void operator()(int x) const { g->n = x; }
  } set{this};
};

int run_in_lambda() {
  int bad = 0;
  [&bad] {
    F f{};
    if (f.self != &f || f.w != 1) bad = 1;
    G g{};
    g.set(9);
    if (g.n != 9) bad = 2;
  }();
  return bad;
}

struct M {
  int base = 7;
  int run() {
    F f{.v = 4};
    if (f.self != &f || f.w != 5) return 10;
    // An explicit initializer's `this` is the member function's object.
    G g{1, {reinterpret_cast<G*>(this)}};
    if (g.set.g != reinterpret_cast<G*>(this)) return 11;
    return 0;
  }
};

// The same in constant evaluation.
constexpr bool in_constexpr() {
  F f{};
  Outer o{};
  G g{};
  g.set(4);
  return f.self == &f && f.w == 1 && o.me == &o && o.f.self == &o.f && g.n == 4;
}
static_assert(in_constexpr());

int main() {
  F f{};
  if (f.self != &f || f.w != 1) return 1;
  F g{2};
  if (g.self != &g || g.w != 3) return 2;
  Outer o{};
  if (o.me != &o || o.f.self != &o.f || o.kk != 10) return 3;
  F* p = new F{};
  if (p->self != p) return 4;
  delete p;
  if (int r = run_in_lambda()) return r;
  M m;
  if (int r = m.run()) return r;
  return 0;
}
