// [class.copy.ctor]/10, [class.copy.assign]/7 (CWG1402): a defaulted move
// constructor or move assignment operator that is defined as deleted is
// ignored by overload resolution, so an rvalue is copied instead (libc++
// `pair<CopyOnly, int>` is move-constructible). An explicitly deleted move
// still participates. Matches clang 19.
//@ expect: run
struct CopyOnly {
  int v = 1;
  constexpr CopyOnly() = default;
  constexpr CopyOnly(const CopyOnly& o) : v(o.v + 10) {}
  CopyOnly(CopyOnly&&) = delete;
  constexpr CopyOnly& operator=(const CopyOnly& o) {
    v = o.v + 100;
    return *this;
  }
  CopyOnly& operator=(CopyOnly&&) = delete;
};
struct Implicit {
  CopyOnly c;
};
template <class T> struct Pair {
  T first;
  int second = 0;
  constexpr Pair() = default;
  constexpr Pair(const Pair&) = default;
  constexpr Pair(Pair&&) = default;
  constexpr Pair& operator=(const Pair&) = default;
  constexpr Pair& operator=(Pair&&) = default;
};
static_assert(__is_constructible(Implicit, Implicit&&));
static_assert(__is_constructible(Pair<CopyOnly>, Pair<CopyOnly>&&));
static_assert(__is_assignable(Pair<CopyOnly>&, Pair<CopyOnly>&&));
static_assert(!__is_constructible(CopyOnly, CopyOnly&&));
static_assert(!__is_assignable(CopyOnly&, CopyOnly&&));
constexpr int moved() {
  Pair<CopyOnly> a;
  Pair<CopyOnly> b(static_cast<Pair<CopyOnly>&&>(a));
  Implicit c;
  Implicit d(static_cast<Implicit&&>(c));
  Pair<CopyOnly> e;
  e = static_cast<Pair<CopyOnly>&&>(b);
  return b.first.v + d.c.v + e.first.v;
}
static_assert(moved() == 11 + 11 + 111);
int main() { return moved() == 133 ? 0 : 1; }
