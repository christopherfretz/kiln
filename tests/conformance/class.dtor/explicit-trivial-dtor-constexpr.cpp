// [class.dtor]/14, /18, [expr.const]/5: an explicit call of a trivial
// destructor (`p->~W()`, `w.~W()`, `p->~T()` with `T` a trivially
// destructible class) runs nothing and is allowed in a constant
// expression; afterwards the storage can be reused (here: by making
// another union member active). Matches clang 19.
//@ expect: compile-ok
struct W { int v; };
struct X { int a, b; };
union U {
  W w;
  X x;
  constexpr U() : w{3} {}
};
template <class T> constexpr void destroy_at(T* p) { p->~T(); }

constexpr int f() {
  U u;
  int r = u.w.v;
  W* p = &u.w;
  p->~W();
  u.x = X{4, 5};
  return r + u.x.b;
}
static_assert(f() == 8);

constexpr int g() {
  U u;
  u.w.~W();
  u.x = X{1, 2};
  destroy_at(&u.x);
  u.w = W{7};
  return u.w.v;
}
static_assert(g() == 7);
