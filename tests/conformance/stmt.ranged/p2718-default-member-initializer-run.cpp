// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5: a default
// member initializer used in an aggregate initialization is part of the
// full-expression, so its temporaries in a for-range-initializer persist
// until the end of the loop, whether the aggregate is written in the call
// or is a default argument (spec/sema.md §7.6a). Each use gets its own
// copy of the member's initializer with fresh temporaries (clang rebuilds
// the `CXXDefaultInitExpr` in a lifetime-extending context). clang 22 and
// GCC 16.1 extend it; clang 19 does not. WP-1177.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
//@ stdout: 1 2 ~D4 | 1 2 ~D4 | end
extern "C" int printf(const char*, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct A {
  int a[2]{1, 2};
  const int* begin() const { return a; }
  const int* end() const { return a + 2; }
};
struct Agg {
  int x = D{4}.v;
};
A ag(const Agg& = Agg{}) { return {}; }
A mk(const Agg&) { return {}; }

int main() {
  for (int x : ag())
    printf("%d ", x);
  printf("| ");
  for (int x : mk(Agg{}))
    printf("%d ", x);
  printf("| end\n");
}
