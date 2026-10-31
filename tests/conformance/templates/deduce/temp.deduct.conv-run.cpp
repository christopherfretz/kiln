// [temp.deduct.conv]: the specialization deduced for each target type
// runs; a conversion to a reference binds to the returned object.
//@ expect: run
//@ stdout: 42 2.5 1 9
//@ xfail(kiln): codegen does not lower UserDefinedConversion casts yet (kiln_codegen)
extern "C" int printf(const char*, ...);
struct A {
  int v = 42;
  template <class T> operator T() const { return T(v); }
};
struct H {
  double d = 2.5;
  template <class T> operator T&() { return d; }
};
struct S {
  int n = 9;
  template <class T> operator T*() { return &n; }
};
int main() {
  A a;
  int i = a;
  H h;
  double& d = h;
  bool b = a;
  S s;
  int* p = s;
  printf("%d %g %d %d\n", i, d, b ? 1 : 0, *p);
  return 0;
}
