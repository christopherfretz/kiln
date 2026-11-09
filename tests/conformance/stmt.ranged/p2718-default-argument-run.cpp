// [class.temporary]/7 (C++23, P2718R0), [intro.execution]/5: the
// temporaries of a default argument used in the for-range-initializer are
// created in it, so they live until the end of the loop too; each use gets
// its own (clang rebuilds the default argument in a lifetime-extending
// context), and a use elsewhere ends with its call's full-expression.
// GCC 16.1 agrees. clang 19 and 22 destroy them at the end of the
// initializer in a template, and clang 22 a nested default argument's
// too. WP-1149; spec/sema-class.md.
//@ expect: run
//@ std: c++23
//@ strict-diags
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not extend default-argument temporaries of a for-range-initializer in a template
//@ stdout: w1 w2 ~D7 | n1 n2 ~D7 | m1 m2 ~D9 ~D8 | ~D7 k7 | t1 t2 ~D7 | t1 t2 ~D7 | end
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
A mk(const D& = D{7}) { return {}; }
int dv(const D& d = D{7}) { return d.v; }
A mkn(int = dv()) { return {}; }
A mk2(const D& = D{8}, const D& = D{9}) { return {}; }

template <class U> void tpl() {
  for (U x : mk()) printf("t%d ", int(x));
  printf("| ");
}

int main() {
  for (int x : mk()) printf("w%d ", x);
  printf("| ");
  // A default argument's own default argument.
  for (int x : mkn()) printf("n%d ", x);
  printf("| ");
  for (int x : mk2()) printf("m%d ", x);
  printf("| ");
  int k = dv();
  printf("k%d | ", k);
  tpl<int>();
  tpl<long>();
  printf("end\n");
}
