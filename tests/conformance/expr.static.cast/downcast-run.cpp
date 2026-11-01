// [expr.static.cast]/2, /11: a downcast moves the pointer or reference back
// by the base subobject's offset; a null pointer stays null, `this` and a
// reference are not null-checked (clang `GetAddressOfDerivedClass`).
//@ expect: run
struct A {
  int a = 1;
};
struct B {
  int b = 2;
};
template <class T> struct D : A, B {
  T d = 3;
};
struct E : D<int> {
  int e = 4;
};
int ptr(B* p) { return static_cast<D<int>*>(p)->d; }
int ref(B& r) { return static_cast<const D<int>&>(r).d + static_cast<E&>(r).e; }
D<int>* keep(B* p) { return static_cast<D<int>*>(p); }
struct F : A, B {
  int f = 6;
  int get() { return static_cast<F*>(static_cast<B*>(this))->f; }
};
int main() {
  E x;
  F y;
  B* null = nullptr;
  if (ptr(&x) != 3 || ref(x) != 7 || keep(null) != nullptr || keep(&x) != &x)
    return 1;
  return y.get() == 6 ? 0 : 2;
}
