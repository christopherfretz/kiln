// [stmt.pre]/1, [stmt.ranged]/1 (C++23, P2360R0): the init-statement of
// a range-based `for` may be an alias-declaration (or a typedef, already
// a simple-declaration), in scope in the range declaration, the range and
// the body; also in a template, naming a dependent type.
//@ expect: run
//@ std: c++23
//@ strict-diags
constexpr int sum() {
  int s = 0, arr[] = {1, 2, 3};
  for (using T = int; T i : arr) s += i;
  for (typedef long L; L i : arr) s += int(i) * sizeof(L) / sizeof(long);
  for (using A = int[2]; int i : A{4, 5}) s += i;
  return s;
}
static_assert(sum() == 21);

template <class C> constexpr long total(const C& c) {
  long s = 0;
  for (using V = typename C::value_type; V v : c.items) s += v;
  return s;
}
struct Box {
  using value_type = short;
  short items[3] = {10, 20, 30};
};
static_assert(total(Box{}) == 60);

int main() {
  int n = 0;
  const char* words[] = {"a", "bc"};
  for (using T = const char*; T p : words) n += p[0];
  return n == 'a' + 'b' ? 0 : 1;
}
