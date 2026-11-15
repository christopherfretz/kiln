// [temp.inst]/11, GNU `used`: implicitly instantiating a class template
// specialization references its non-template member functions that carry
// `used` (clang `VisitCXXMethodDecl`, "PR17480: Honor the used attribute
// to instantiate member function definitions"), so they are instantiated
// at the end of the translation unit and emitted `linkonce_odr` in
// `@llvm.compiler.used` (`@llvm.used` with `retain`), called or not. Not
// referenced: a member function template, a friend, a static data member
// and the members of a member class that is not instantiated, or of a
// specialization only named (`E<char> *`). Under an explicit
// instantiation declaration nothing is emitted (`available_externally`
// only when optimizing). In-class `used` definitions of a non-template
// class and of an explicit specialization are emitted after the class
// (clang `HandleInlineFunctionDefinition`). A variable template's and an
// out-of-line member definition's `used` carries to the instantiation.
// Found in downstream code (debug-dump members of class templates).
//@ expect: ir
template <class T> struct S {
  __attribute__((used)) S() {}
  __attribute__((used)) int op() const { return 2; }
  __attribute__((used, retain)) int r() { return 8; }
  __attribute__((used)) int defd();
  __attribute__((used)) static int sm() { return 1; }
  [[gnu::used]] operator int() const { return 3; }
  int plain() { return 4; }
  template <class U> int mt(U) { return 5; }
  friend __attribute__((used)) int fr(S) { return 6; }
  static int sd;
  [[gnu::used]] static inline int si = 3;
  struct N { __attribute__((used)) int nf() { return 4; } };
  struct M { __attribute__((used)) int mf() { return 5; } };
};
template <class T> int S<T>::defd() { return 9; }
template <class T> __attribute__((used)) int S<T>::sd = 7;
S<long>::N n;
template <class T> __attribute__((used)) int ft(T) { return 1; }
template <class T> __attribute__((used)) int vt = 1;
int use() { return ft(1) + vt<int> + S<char>::sd; }
namespace {
template <class T> struct A { __attribute__((used)) int af() { return 5; } };
A<int> a;
}
template <class T> struct E { __attribute__((used)) int ef() { return 6; } };
template <> struct E<int> { __attribute__((used)) int ef() { return 7; } };
E<int> e;
E<char> *ep;
template <class T> struct V { __attribute__((used)) int vf() { return 1; } };
extern template struct V<int>;
V<int> v;
struct P {
  static int sd;
  struct I { __attribute__((used)) int i() { return 2; } };
  __attribute__((used)) P() {}
  __attribute__((used)) ~P() {}
  __attribute__((used)) int f() { return 1; }
  friend __attribute__((used)) int pf(P) { return 3; }
  __attribute__((used)) static inline int b = 5;
};
int P::sd = 1;
template <class T> struct W {
  __attribute__((used)) W() {}
  __attribute__((used)) int wf() { return 1; }
};
template struct W<int>;
