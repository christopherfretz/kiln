// [temp.friend]/1, [temp.param]/12, [temp.spec.partial.match]/1,
// [temp.expl.spec]/1, [temp.explicit]/1, [temp.arg.template]/3,
// [temp.class.general]/3: a class template first declared as a friend
// inside a class template (whose parameters sit one level deeper) is used
// with the namespace scope declaration's parameters and defaults: partial
// and explicit specializations, an explicit instantiation, out-of-line
// member definitions, a template template argument (relaxed matching needs
// the default), deduction through `TT<A, B>` and of a partial
// specialization's argument, and partial ordering of two function
// templates naming it. Verified against clang 19 (WP-299 audit).
//@ expect: run
//@ exit: 0
template <class C> struct traits { static constexpr int v = sizeof(C); };
template <class I> class wrap {
  template <class X, class Y> friend class sv;
  template <class X, int N> friend struct arr;
};
template <class C, class Tr = traits<C>> class sv {
public:
  constexpr int get() const;
  template <class X> constexpr int mem(X) const;
  static const int s;
  constexpr int self() const { sv<C> o; sv p = o; return p.get(); }
};
template <class C, class Tr> constexpr int sv<C, Tr>::get() const { return Tr::v; }
template <class C, class Tr>
template <class X>
constexpr int sv<C, Tr>::mem(X) const { return sizeof(X) + Tr::v; }
template <class C, class Tr> const int sv<C, Tr>::s = 7;
template <class Tr> class sv<long, Tr> { public: constexpr int get() const { return 50; } };
template <> class sv<short> { public: constexpr int get() const { return 60; } };
template class sv<int>;
template <class C, int N = sizeof(C) * 3> struct arr { static constexpr int n = N; };
template <class C> struct arr<C*> { static constexpr int n = -1; };

template <template <class> class TT> struct usett { static constexpr int v = TT<char>{}.get(); };
template <template <class> class TT> struct usett4 { static constexpr int v = TT<short>::n; };
template <template <class, class> class TT, class A, class B>
constexpr int ded2(const TT<A, B>& s) { return s.get(); }
template <class T> constexpr int f(arr<T>) { return arr<T>::n; }
template <class T, int M> constexpr int g(arr<T, M>) { return M; }
template <class C, class D> constexpr int ord(sv<C>, D) { return 1; }
template <class C> constexpr int ord(sv<C>, int) { return 2; }

static_assert(sv<char>{}.get() == 1);
static_assert(sv<long>{}.get() == 50);
static_assert(sv<short>{}.get() == 60);
static_assert(sv<char>{}.mem(1) == 5);
static_assert(sv<int>::s == 7);
static_assert(sv<char>{}.self() == 1);
static_assert(usett<sv>::v == 1);
static_assert(usett4<arr>::v == 6);
static_assert(arr<int*>::n == -1);
static_assert(ded2(sv<char>{}) == 1);
static_assert(f(arr<char>{}) == 3);
static_assert(g(arr<int>{}) == 12);
static_assert(ord(sv<char>{}, 1) == 2 && ord(sv<char>{}, 1.0) == 1);
int main() {}
