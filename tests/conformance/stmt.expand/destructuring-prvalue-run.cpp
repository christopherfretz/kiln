//@ expect: run
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects (and clang 22 lacks expansion statements)
//@ stdout: 0 8 1 3 1
// [stmt.expand]/5.3, [expr.const.init]/2 (C++26, P2686R5): a `constexpr`
// destructuring expansion over a prvalue binds its lifetime-extended
// temporary by `constexpr auto&& [u...]`, which is constexpr-referenceable
// in this function: an empty one is accepted, a `constexpr auto&&`
// element may refer to a member, and that member is the temporary's own
// at run time (one object, not a copy per element); without `constexpr`
// the elements are read as usual. GCC 16.1 rejects the `constexpr` ones
// (no P2686). The rejected side: `destructuring-prvalue-neg.cpp`.
// Verdict for the empty prvalue range `E{}` (WP-1525): accepted. By
// [stmt.expand] the expansion is `constexpr auto&& [u0, ..., uN-1] =
// E{};` with N = 0 (the structured binding size of `E`); the reference
// binds the lifetime-extended temporary, constexpr-referenceable in this
// function ([expr.const.init]), so the declaration is a constant
// initialization, and no element reads the temporary. GCC 16.1's
// "'<anonymous>' is not a constant expression" is its missing P2686.
extern "C" int printf(const char *, ...);
struct E {};
struct P { int a = 1; int b = 2; };
int main() {
  int n = 0;
  template for (constexpr auto e : E{}) { n += 100; }
  unsigned long sz = 0;
  const int *first = nullptr;
  int adjacent = 0;
  template for (constexpr auto &&x : P{}) {
    sz += sizeof(x);
    if (first)
      adjacent = &x == first + 1;
    first = &x;
  }
  int s = 0;
  template for (auto x : P{}) { s += x; }
  printf("%d %lu %d %d %d\n", n, sz, adjacent, s, first != nullptr);
}
