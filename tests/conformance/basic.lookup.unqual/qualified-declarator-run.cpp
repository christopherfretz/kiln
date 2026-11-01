// [basic.lookup.unqual]/1 (and [class.mfct]/5, [temp.mem.func]/1): names
// used after a qualified declarator-id are looked up in the qualifier's
// scope — member typedefs, the injected-class-name, namespace members.
//@ expect: run
//@ stdout: 3 7 12 5 9 1 4
extern "C" int printf(const char *, ...);

struct S {
  using vt = int;
  static vt f(vt);
  vt g(const S &) const;
  int k = 3;
};
S::vt S::f(vt x) { return x; }
S::vt S::g(const S &o) const { return o.k; }

namespace N {
struct T { int v; };
int f(T);
}
int N::f(T t) { return t.v; }

template <class U> struct A {
  using vt = U;
  vt get(const A &) const;
  typename A<U>::vt twice(vt);
  static vt make(vt);
  U u;
};
template <class U> typename A<U>::vt A<U>::get(const A &o) const { return o.u; }
template <class U> typename A<U>::vt A<U>::twice(vt x) { return x + x; }
template <class U> U A<U>::make(vt x) { return x - 1; }

template <class P> struct Q;
template <class P> struct Q<P *> {
  using vt = P;
  static vt deref(P *);
};
template <class P> typename Q<P *>::vt Q<P *>::deref(P *p) { return *p; }

int main() {
  S s;
  A<int> a{9};
  int one = 1;
  printf("%d %d %d %d %d %d %d\n", s.g(s), S::f(7), a.twice(6),
         A<int>::make(6), a.get(a), Q<int *>::deref(&one),
         N::f(N::T{4}));
}
