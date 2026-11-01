// [temp.point]/7, [temp.inst]/5: an implicit instantiation of a function
// (template specialization or member of a class template specialization)
// odr-used by a non-template definition is instantiated at the end of the
// translation unit. Codegen mirrors clang: such a definition is handed to
// EmitGlobal only then (after the TU's own definitions and key-function
// vtables), so it is queued behind inline functions referenced later in
// the same body, and the functions its body references are declared
// after theirs (spec/codegen.md §2a).
inline int q(int x) { return x; }
inline int r(int x) { return x; }
inline int s(int x) { return x; }
inline int t(int x) { return x; }

// g<int> is referenced before i1, but emitted after it: r follows q.
template <class T> T g(T x) { return r(x); }
inline int i1(int x) { return q(x) + 2; }
int h(int x) { return g(x) + i1(x); }

// A static data member definition of a specialization with a dynamic
// initializer is emitted when instantiated (after the key-function vtable
// of K), not where the TU walk would meet it.
struct K { virtual void k(); };
void K::k() {}
template <class T> struct D {
  static int cnt;
  int get() { return cnt; }
};
template <class T> int D<T>::cnt = r(3);
template <class T> int use(T x) { D<T> d; return d.get() + x; }
int main() { return use(1); }

// A constexpr function template is instantiated at its use (not held), a
// template defined after its use, a member template, a generic lambda,
// a deduced return type.
template <class T> constexpr T ce(T x) { return x + q(1); }
template <class T> T late(T x);
struct M {
  template <class T> int mt(T x) { return s(x); }
  int nm() { return t(1); }
};
template <class T> struct C {
  int a() { return q(2); }
  int b();
  template <class U> int c(U u) { return r(u); }
};
template <class T> int C<T>::b() { return s(3); }
int gv = g(4) + ce(5);
int h2(int x) {
  M m;
  C<int> c;
  return late(x) + m.nm() + c.b() + m.mt(x) + c.a() + ce(x) + c.c(2L) +
         [](int y) { return g(y); }(x);
}
template <class T> T late(T x) { return t(x) + g(x); }
template <class T> auto au(T x) { return x + s(2); }
int k2() {
  auto l = [](auto z) { return q(z); };
  return l(1) + au(3);
}

// Virtual members and structors of specializations.
struct Count {
  virtual ~Count();
  virtual int on_zero() noexcept = 0;
  int z() { return s(1); }
};
Count::~Count() {}
template <class T> struct P : Count {
  T v;
  P(T x) : v(x) {}
  ~P() override { r(1); }
  int on_zero() noexcept override { return q(v) + z(); }
};
int drop() { Count *c = new P<long>(2); int v = c->on_zero(); delete c; return v; }
