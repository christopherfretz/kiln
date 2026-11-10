// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5,
// [dcl.init.aggr]/5: the default member initializers an aggregate
// initialization in a for-range-initializer uses are part of its
// full-expression, so their temporaries persist until the end of the loop:
// in a nested aggregate (written, or initialized from `{}` by its own
// member's absence), in a default argument's aggregate, in a partial or
// designated initializer list, in both arms of a conditional, in a class
// template's and a function template's aggregate, and when the initializer
// uses `this`. Each use gets its own copy of the member's initializer with
// fresh temporaries (spec/sema.md §7.6a). Like clang 22, a member
// initializer whose own full-expression holds no temporary is not rebuilt,
// so the temporaries of the default member initializers it uses in turn
// stay its own (`O2{}`: `~D4` before the loop). WP-1177.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ requires: clang>=22
//@ stdout: 1 2 ~D5 ~D4 | 1 2 ~D5 ~D4 | 1 2 ~D4 | 1 2 ~D4 | 1 2 ~D4 | 1 2 ~D4 | 1 2 ~D9 | 1 2 ~D5 ~D4 | 1 2 ~D11 | ~D4 1 2 ~D5 | end
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
struct Outer {
  Agg in;
  int y = D{5}.v;
};
struct Big {
  int z;
  Agg g;
};
template <class T> struct TA {
  T x = D{9}.v;
};
struct S {
  int a = 1;
  int b = D{a + 10}.v + a;
};
struct O2 {
  Agg a = Agg{};
  int y = D{5}.v;
};
template <class T> A mk(const T&) { return {}; }
A mo(const Outer& = Outer{}) { return {}; }
template <class T> void tpl() {
  for (int x : mk(T{}))
    printf("%d ", x);
  printf("| ");
}

int main(int argc, char**) {
  for (int x : mk(Outer{}))
    printf("%d ", x);
  printf("| ");
  for (int x : mo())
    printf("%d ", x);
  printf("| ");
  for (int x : mk(Big{1}))
    printf("%d ", x);
  printf("| ");
  for (int x : mk(Big{.z = 1}))
    printf("%d ", x);
  printf("| ");
  for (int x : mk(argc > 5 ? Agg{} : Agg{}))
    printf("%d ", x);
  printf("| ");
  tpl<Agg>();
  for (int x : mk(TA<int>{}))
    printf("%d ", x);
  printf("| ");
  for (int x : mk(Outer{Agg{}}))
    printf("%d ", x);
  printf("| ");
  for (int x : mk(S{}))
    printf("%d ", x);
  printf("| ");
  for (int x : mk(O2{}))
    printf("%d ", x);
  printf("| end\n");
}
