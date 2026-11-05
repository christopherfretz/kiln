// [expr.prim.fold] (C++17, N4295 / P0036R0): unary and binary left and
// right folds, empty-pack identities. WP-646 feature probe.
//@ expect: run
//@ std: c++17
//@ strict-diags
//@ stdout: 10 -2 1 0 abc 4
extern "C" int printf(const char*, ...);
template <class... T> constexpr auto sum(T... t) { return (t + ...); }
template <class... T> constexpr int lsub(T... t) { return (... - t); }
template <class... T> constexpr int rsub(T... t) { return (t - ...); }
template <class... T> constexpr bool all(T... t) { return (t && ...); }
template <class... T> constexpr bool any(T... t) { return (... || t); }
template <class... T> constexpr int init(T... t) { return (100 - ... - t); }
template <class... T> void print(T... t) { (printf("%s", t), ...); }
template <class... T> constexpr int count(T...) { return (0 + ... + sizeof(T)); }
static_assert(sum(1, 2, 3, 4) == 10 && lsub(1, 2, 3) == -4 && rsub(1, 2, 3) == 2);
static_assert(all() && !any() && init(1, 2) == 97);
int main() {
  printf("%d %d %d %d ", sum(1, 2, 3, 4), rsub(1, 2, 3) - 4, all(1, 2), any(0, 0));
  print("a", "b", "c");
  printf(" %d\n", count(1, 'c') - 1);
}
