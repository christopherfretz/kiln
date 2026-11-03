//@ args: -O1 -Xclang -disable-llvm-passes
// [temp.explicit]/12, [class.virtual]: above -O0 the vtable of a class
// whose vtable another translation unit defines (an explicit
// instantiation declaration, or a key function not defined here) is
// emitted `available_externally` when it can be speculatively (clang
// `shouldEmitAvailableExternallyVTable` / `canSpeculativelyEmitVTable`):
// every inline virtual function in it must already be emitted, and for
// a class with virtual bases the same must hold for its bases. Its RTTI
// stays external, and its thunks of methods defined elsewhere are
// defined `available_externally` with it.

// An inline virtual function that is never emitted: the vtables stay
// external (A, C), as does that of a class with a virtual base whose
// vtable cannot be emitted (D).
template <class T> struct A {
  A() {}
  virtual ~A() {}
  virtual T f() { return T(); }
};
// The only inline virtual function is used, so emitted.
template <class T> struct B {
  B() {}
  virtual T f();
  virtual T g() { return T(1); }
};
template <class T> T B<T>::f() { return T(); }
template <class T> struct C {
  C() {}
  virtual T f() { return T(); }
  virtual T h();
};
template <class T> T C<T>::h() { return T(); }
template <class T> struct D : virtual A<T> {
  D() {}
  virtual T k() { return T(); }
};
extern template struct A<int>;
extern template struct B<int>;
extern template struct C<int>;
extern template struct D<int>;
void use() {
  A<int> a;
  B<int> b;
  C<int> c;
  D<int> d;
  b.g();
}

// Virtual bases whose vtables can be emitted: the VTT is emitted
// `available_externally` too.
template <class T> struct ios { virtual ~ios(); };
template <class T> struct istr : virtual ios<T> {
  istr();
  virtual int get();
};
extern template struct ios<char>;
extern template struct istr<char>;
int use_istr() {
  istr<char> s;
  return s.get();
}

// Key functions defined elsewhere.
struct K { virtual int f(); };
int use_k() {
  K k;
  return k.f();
}
// An inline virtual function emitted only after the vtable was first
// considered (clang `EmitVTablesOpportunistically`).
struct J {
  virtual int k();
  virtual int g() { return 1; }
};
inline int call_g(J &j) { return j.J::g(); }
int use_j() {
  J j;
  return call_g(j) + j.k();
}
// `this`-adjusting thunks, covariant return thunks and virtual-base
// adjusting thunks of methods defined elsewhere.
struct X { virtual int f(); virtual ~X(); };
struct Y { virtual int f(); };
struct G : X, Y { int f() override; ~G(); };
int use_g() {
  G g;
  return g.f();
}
struct V { virtual int v(); virtual ~V(); };
struct L : virtual V { int v() override; virtual L *cov(); ~L(); };
struct M : virtual V, L { M(); int v() override; M *cov() override; ~M(); };
int use_m() {
  M m;
  return m.v();
}
