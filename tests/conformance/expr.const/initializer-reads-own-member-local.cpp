// [expr.const]/10.9, [basic.life]/7: an automatic variable of a constexpr
// function (constexpr or not) may read its already-initialized members in
// its own initializer during constant evaluation; the name designates the
// object under construction. WP-1100.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct P { int x; int y; };
struct N { P a; int b; };
constexpr int local() {
  P l{3, l.x * 2};
  return l.y;
}
static_assert(local() == 6);
constexpr int local_constexpr() {
  constexpr P l{3, l.x * 2};
  return l.y;
}
static_assert(local_constexpr() == 6);
constexpr int nested() {
  N l{{1, l.a.x + 1}, l.a.y * 10};
  return l.b;
}
static_assert(nested() == 20);
struct U { int a; int b = a; };
constexpr int nsdmi() {
  U u{7};
  return u.b;
}
static_assert(nsdmi() == 7);
// A recursive call names its own frame's object, not the caller's.
constexpr int rec(int k) {
  P l{k, k ? rec(k - 1) + l.x : 0};
  return l.y;
}
static_assert(rec(3) == 6);
