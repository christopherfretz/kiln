// [expr.const]/16, [stmt.expand]/5.1: an immediate invocation in an
// element of an enumerating expansion-init-list is evaluated at the end of
// the full-expression of the declaration that element initializes, in a
// function and in a function template. Kiln built the elements before
// their declarations and never evaluated them, so the program called the
// consteval function at run time and failed to link. A temporary inside the
// immediate invocation ends with it ([class.temporary]/8 does not extend
// it). GCC 16.1 agrees; clang 22 has no P1306. WP-1106; spec/sema.md §7.6a.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: 4 5 6 | 7 8 | 9 3 |
extern "C" int printf(const char*, ...);

struct C {
  int v;
  constexpr ~C() {}
};

consteval int jv(int c) { return c; }
consteval int iv(const C& c) { return c.v; }

template <class T>
void g() {
  template for (auto x : {jv(7), T(8)}) printf("%d ", x);
  printf("| ");
}

constexpr int sum() {
  int s = 0;
  template for (auto x : {iv(C{1}), jv(2)}) s += x;
  return s;
}

int main() {
  template for (auto x : {jv(4), 5, iv(C{6})}) printf("%d ", x);
  printf("| ");
  g<int>();
  template for (const auto& x : {iv(C{9}), sum()}) printf("%d ", x);
  printf("|\n");
}
