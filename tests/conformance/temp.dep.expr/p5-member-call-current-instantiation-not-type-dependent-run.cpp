// [temp.dep.expr]/5, /1, /3.3: `this->fn(42)`, an implicit `this->mf()`,
// and `A0::nf()` naming a member function of the current instantiation or
// of a non-dependent base with a non-dependent type refer to members of
// the current instantiation whose type is not dependent: the calls are not
// type-dependent, so `auto b = this->fn(42);` has the non-dependent type
// `B`, and `b.bfn<int>()` names the member template of `B` without
// `template` ([temp.names]/7.3.2). A static member function called
// without `this` already works. Found running GCC's C++ testsuite
// (`lookup/member3`-`member5` shapes). Clang 22 treats a member call
// through a type-dependent `this` as type-dependent; GCC 16.1 accepts all
// but `mf()` (an implicit `this` calling a non-static member of the
// class itself), so the text decides and no oracle is compared.
//@ expect: run
//@ skip(clang): clang 22 treats a call through `this` of the current instantiation as type-dependent
struct B {
  template <class> int bfn() { return 1; }
};
struct A0 {
  static B sf() { return B(); }
  B nf() { return B(); }
};
template <typename T> struct A : A0 {
  static B fn(int) { return B(); }
  B mf() { return B(); }
  int g() {
    auto b1 = fn(42);
    auto b2 = this->fn(42);
    auto b3 = mf();
    auto b4 = sf();
    auto b5 = nf();
    auto b6 = A0::nf();
    auto b7 = this->A0::nf();
    return b1.bfn<int>() + b2.bfn<int>() + b3.bfn<int>() + b4.bfn<int>() +
           b5.bfn<int>() + b6.bfn<int>() + b7.bfn<int>();
  }
};
int main() { return A<int>().g() == 7 ? 0 : 1; }
