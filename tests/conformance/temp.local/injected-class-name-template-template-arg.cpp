// [temp.local]/1: the injected-class-name of a class template (or of a
// partial specialization, or of a member class template) used as a
// template-argument for a template template parameter names the class
// template, also after instantiation (libc++ 22 `__split_buffer`
// passes `__split_buffer_pointer_layout` from inside itself). Used for
// a type parameter it is the current specialization.
//@ expect: compile-ok
template <template <class> class L> struct H { static constexpr int v = 1; };
template <template <class> class... Ls> struct HP { static constexpr int n = sizeof...(Ls); };
template <class T, template <class> class L> struct HT { using type = T; };
template <template <class> class L> using A = H<L>;
template <template <class> class L> constexpr int V = H<L>::v;
template <class T> struct X {
  H<X> a;
  HP<X, X> b;
  HT<X, X>::type* c;
  A<X> d;
  static constexpr int e = V<X>;
  template <class U> struct M { H<M> m; };
};
X<int> x;
X<int>::M<char> xm;
static_assert(__is_same(decltype(x.a), H<X>));
static_assert(__is_same(decltype(x.c), X<int>*));
static_assert(decltype(x.b)::n == 2);
template <class T> struct P;
template <class T> struct P<T*> { H<P> h; };
P<int*> pp;
static_assert(__is_same(decltype(pp.h), H<P>));
