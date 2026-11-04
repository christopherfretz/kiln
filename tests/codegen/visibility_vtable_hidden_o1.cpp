//@ args: -O1 -Xclang -disable-llvm-passes -fvisibility=hidden
// isVTableHidden (spec/codegen.md "Symbol visibility", "Vtable
// emission"; clang canSpeculativelyEmitVTableAsBase): above -O0 a vtable
// defined in another translation unit is not emitted
// `available_externally` when it would be hidden, whether by the class's
// visibility or because a slot's method is hidden and not defined here;
// it stays an external declaration.
template <class T> struct A {
  A() {}
  virtual T f() { return T(); }
  virtual T g();
};
template <class T> T A<T>::g() { return T(); }
extern template struct A<int>;
struct __attribute__((visibility("default"))) K {
  K() {}
  virtual int k();
  virtual int i() { return 1; }
};
struct __attribute__((visibility("default"))) M {
  M() {}
  virtual int m();
  __attribute__((visibility("hidden"))) virtual int h();
  virtual int j() { return 2; }
};
int use() {
  A<int> a;
  K k;
  M m;
  return a.f() + k.i() + m.j();
}
