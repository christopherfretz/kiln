// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5: a default
// argument used by a default member initializer that an aggregate
// initialization in a for-range-initializer uses is part of the same
// full-expression, so its temporary persists until the end of the loop.
// kiln rebuilds the member initializer and its nested default argument for
// the use (spec/sema.md §7.6a), as it does a nested default argument. GCC
// 16.1 agrees; clang 22 destroys a nested default argument's temporary at
// the end of the initializer. WP-1177.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ oracle: gcc
//@ skip(clang): clang 22 does not extend a nested default argument's temporaries in a for-range-initializer
//@ stdout: 1 2 ~D8 | end
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
int dv(const D& d = D{8}) { return d.v; }
struct Agg {
  int x = dv();
};
A mk(const Agg&) { return {}; }

int main() {
  for (int x : mk(Agg{}))
    printf("%d ", x);
  printf("| end\n");
}
