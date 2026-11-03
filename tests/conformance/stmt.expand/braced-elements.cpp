// [stmt.expand]/5.1 (P1306R5): a braced element of an expansion-init-list
// initializes the for-range-declaration as `decl = {...};` would
// (copy-list-initialization, [dcl.init.list]/3): aggregates with nested
// and designated lists ([dcl.init.list]/3.1, [dcl.init.aggr]), a
// reference bound to the list's temporary for its element's lifetime,
// `auto` deducing std::initializer_list per element ([dcl.type.auto.deduct]/
// 2.1), braced elements in templates and as a pack expansion `{I, I}...`
// ([temp.variadic]/5.4), and nested expansions. The Oven runs each
// element's temporaries' destructors at the end of that element's block.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
#include <initializer_list>
struct P { int x, y; };
struct Q { P p; int z; };
template <class T> constexpr int g(T t) {
  int s = 0;
  template for (P p : {{t, 2}, {.y = t}}) { s += p.x * 10 + p.y; }
  return s;
}
static_assert(g(3) == 35);
template <int... I> constexpr int h() {
  int s = 0;
  template for (P p : {{I, I}...}) { s += p.x * p.y; }
  return s;
}
static_assert(h<1, 2, 3>() == 14);
template <int... I> constexpr int h2() {
  int s = 0;
  template for (P p : {{I, 1}..., {7, 7}}) { s += p.x * p.y; }
  return s;
}
static_assert(h2<1, 2>() == 52);
constexpr int k() {
  int s = 0;
  template for (Q q : {{{1, 2}, 3}, {.p = {.x = 4}, .z = 5}, {}}) { s += q.p.x + q.p.y + q.z; }
  return s;
}
static_assert(k() == 15);
constexpr int il() {
  int s = 0;
  template for (auto x : {{1, 2}, {3, 4, 5}}) { s += x.size() * 10 + *x.begin(); }
  return s;
}
static_assert(il() == 54);
constexpr int cr() {
  int s = 0;
  template for (const P& p : {{1, 2}, {3}}) { s += p.x + p.y; }
  return s;
}
static_assert(cr() == 6);
constexpr int mixed() {
  int s = 0;
  template for (auto x : {1, 2L}) { s += sizeof(x); }
  template for (P p : {P{1, 1}, {2, 2}}) { s += p.x; }
  return s;
}
static_assert(mixed() == sizeof(int) + sizeof(long) + 3);
struct Log { int buf[16]{}; int n = 0; constexpr void put(int v) { buf[n++] = v; } };
struct D {
  Log* l; int v;
  constexpr D(Log* l, int v) : l(l), v(v) { l->put(v); }
  constexpr ~D() { l->put(-v); }
};
constexpr bool order() {
  Log l;
  template for (const D& d : {{&l, 1}, {&l, 2}}) { l.put(d.v * 10); }
  int want[] = {1, 10, -1, 2, 20, -2};
  for (int i = 0; i < 6; ++i) if (l.buf[i] != want[i]) return false;
  return l.n == 6;
}
static_assert(order());
template <class T> constexpr int nest() {
  int s = 0;
  template for (T t : {{1, 2}, {3, 4}}) {
    template for (int i : {{t.x}, {t.y}}) { s += i; }
  }
  return s;
}
static_assert(nest<P>() == 10);
