// [expr.ref]/7.1, [expr.const]: `E1.m` and `E1->m` naming a static data
// member designate that member; `E1` is still evaluated (its side effects
// happen) but no object is read through it, so a null `E1->m` is fine.
// A static data member of reference type designates its referent.
//@ expect: compile-ok
struct S {
  static constexpr int extent = 3;
  int x;
  static constexpr bool eq(char a, char b) { return a == b; }
};
constexpr bool f() { S s{1}; return s.extent == 3; }
static_assert(f());
template <class P> constexpr bool call(P p) { return p('a', 'a'); }
constexpr bool g() { return call(S::eq); }
static_assert(g());
template <class T> constexpr bool h() { return call(T::eq); }
static_assert(h<S>());
struct R { static constexpr const int &r = S::extent; };
constexpr int side() {
  int n = 0;
  R x;
  S *p = nullptr;
  (void)(++n, x).r;
  return p->extent + x.r + n;
}
static_assert(side() == 7);
