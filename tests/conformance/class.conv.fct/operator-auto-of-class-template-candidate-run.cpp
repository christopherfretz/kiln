//@ expect: run
//@ std: c++20
//@ stdout: 3 3 4 4 5 7 6
// [dcl.spec.auto.general]/12: the return type of a conversion function of
// a class template specialization, `operator auto`, is deduced when its
// definition is instantiated; considering it as a candidate
// ([over.match.copy], [over.match.ref]) instantiates it, as clang does.
extern "C" int printf(const char *, ...);

struct T {
  int v;
  T(int x) : v(x) {}
  T(T &&) = default;
};
template <class X> struct E {
  operator auto() && { return T{3}; }
};
template <class X> struct F {
  explicit operator auto() && { return T{4}; }
};

template <class V> struct C;
template <class V> struct Base {
  // Inherited, and converts to another specialization of the class.
  explicit operator auto() && {
    return C<V &&>{static_cast<V &&>(*static_cast<C<V> *>(this)->p)};
  }
};
template <class V> struct C : Base<V> {
  V *p;
  C(V &x) : p(&x) {}
  C(C &&) = default;
};
template <class V> struct C<V &&> {
  V *p;
  C(V &&x) : p(&x) {}
  C(C &&) = default;
};

template <class X> struct G {
  operator auto() const { return X{6}; }
};
int take(int v) { return v; }

int main() {
  T a = E<int>{};
  T b(E<long>{});
  T c(F<int>{});
  T d{F<char>{}};
  int i = 5;
  C<int> ci{i};
  auto r = C<int &&>{static_cast<C<int> &&>(ci)};
  int j = 7;
  C<int> cj{j};
  auto r2 = C<int &&>(static_cast<C<int> &&>(cj));
  printf("%d %d %d %d %d %d %d\n", a.v, b.v, c.v, d.v, *r.p, *r2.p, take(G<int>{}));
}
