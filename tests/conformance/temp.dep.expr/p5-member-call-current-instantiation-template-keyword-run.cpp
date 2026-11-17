// [temp.dep.expr]/5, [temp.names]/3 and /7: once `this->mf()` and
// `mf()` (members of the current instantiation with a non-dependent
// type) are not type-dependent, the object of `.v` is not type-dependent
// either, so lookup of `v` finds the member template and the `<` after it
// begins a template argument list: `this->mf().v < 1 > (2)` is `v<1>(2)`,
// not `(... .v < 1) > (2)`. A `template` keyword that is now redundant
// stays valid ([temp.names]/7 allows it before any member template name).
// Clang 22 treats these calls as type-dependent and reads the `<` as
// less-than (an error at instantiation); GCC 16.1 also reads them as
// dependent, so the text decides and no oracle is compared.
//@ expect: run
//@ skip(clang): clang 22 treats a call through `this` of the current instantiation as type-dependent
struct B {
  template <class> int bfn() { return 1; }
  template <int N> int v(int x) { return N + x; }
};
struct A0 {
  B nf() { return B(); }
};
template <typename T> struct A : A0 {
  B mf() { return B(); }
  static B fn(int) { return B(); }
  int g() {
    auto b = this->mf();
    int t = b.template bfn<int>() + this->fn(1).template bfn<T>() +
            nf().template bfn<int>();
    return t + this->mf().v < 1 > (2) + mf().v<2>(0);
  }
};
int main() { return A<int>().g() == 3 + 3 + 2 ? 0 : 1; }
