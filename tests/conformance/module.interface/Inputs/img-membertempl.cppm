// Member templates and redeclared templates for the declaration image
// (spec/modules.md §11.11 "Member templates", WP-1292).
export module imgmt;
export namespace mt {
// Member templates of a class: a function template defined in the class,
// a static one defined out of line, a member class template.
struct A {
  template <class U> int f(U u) const { return sizeof(u); }
  template <class U> static U g();
  template <class U> struct In {
    U u;
    int get() const { return (int)u; }
  };
  template <int N> static constexpr int k() { return N * 2; }
};
template <class U> U A::g() { return U(7); }

// Member templates of a class template (depth 1), one defined out of
// line, a member class template with a member template of its own
// (depth 2) defined out of line, and a member class template defined
// out of line.
template <class T> struct S {
  T t;
  template <class U> T add(U u) const { return t + T(u); }
  template <class U> T mul(U u) const;
  template <class V> struct In {
    V v;
    T w;
    T sum() const { return w + T(v); }
    template <class W> T tri(W x) const;
  };
  template <class V> struct Out;
};
template <class T> template <class U> T S<T>::mul(U u) const { return t * T(u); }
template <class T>
template <class V>
template <class W>
T S<T>::In<V>::tri(W x) const {
  return w + T(v) + T(x);
}
template <class T> template <class V> struct S<T>::Out {
  V v;
  T get() const { return T(v) + 1; }
};

// A template declared, then defined: the definition inherits the
// declaration's default arguments ([temp.param]/12).
template <class T = int, int N = 3> T scaled(T x);
template <class T, int N> T scaled(T x) { return x * N; }
template <class T = long> struct D;
template <class T> struct D {
  static constexpr int size = sizeof(T);
};

// An error inside a member template's pattern, met by instantiation.
template <class T> struct Bad {
  template <class U> static int h(U u) { return u.missing; }
};
} // namespace mt
