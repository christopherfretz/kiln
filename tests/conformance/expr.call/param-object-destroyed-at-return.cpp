// [expr.call]/7: clang ends a parameter object's lifetime, running its
// destructor, when the call returns (last constructed first), before the
// rest of the full-expression; temporaries bound to reference parameters
// live on to its end ([class.temporary]/6.2). Kiln destroyed parameter
// objects at the end of the full-expression and baked the wrong values
// (WP-1018). Same at -fclang-compat=19 and 22.
//@ std: c++23
struct L {
  int *log;
  int id;
  constexpr ~L() { *log = *log * 10 + id; }
};
constexpr int take(L, L) { return 0; }
constexpr int take_ref(L, const L &) { return 0; }
constexpr int t1() {
  int log = 0;
  int at = (take(L{&log, 1}, L{&log, 2}), log);
  return at * 1000 + log;
}
constexpr int t2() {
  int log = 0;
  int at = (take_ref(L{&log, 1}, L{&log, 3}), log);
  return at * 1000 + log;
}
struct C {
  int n;
  constexpr C(L l) : n(l.id) {}
};
constexpr int t3() {
  int log = 0;
  C c(L{&log, 4});
  return log * 10 + c.n;
}
// A reference parameter's temporary outlives the call.
constexpr const int *keep(L, const int &x) { return &x; }
constexpr int t4() {
  int log = 0;
  return *keep(L{&log, 5}, 9) + log;
}
static_assert(t1() == 21021);
static_assert(t2() == 1013);
static_assert(t3() == 44);
static_assert(t4() == 14);
