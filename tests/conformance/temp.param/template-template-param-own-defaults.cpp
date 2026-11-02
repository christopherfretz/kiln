// [temp.param]/14, [temp.arg.template]/3: a template template parameter's own
// default template arguments apply to template-ids naming that parameter
// (`TT<int>` with `template <class, int = 5> class TT`), independent of the
// argument template's defaults. A default may name the parameter's earlier
// parameters (`class = T*`) or an enclosing template's parameters.
//@ expect: run
template <template <class, int = 5> class TT> struct S { TT<int> x; };
template <class T, int N> struct A { static constexpr int n = N; T v[N]; };
static_assert(decltype(S<A>::x)::n == 5);
static_assert(sizeof(S<A>) == 5 * sizeof(int));

template <class X, class Y> struct B {};
template <template <class, class = int> class C> struct U { using type = C<char>; };
static_assert(__is_same(U<B>::type, B<char, int>));

template <template <class T, class = T*> class C> struct D { using type = C<long>; };
static_assert(__is_same(D<B>::type, B<long, long*>));

template <class T> struct Out {
  template <template <class, class = T> class C> struct In { using type = C<short>; };
};
static_assert(__is_same(Out<float>::In<B>::type, B<short, float>));

// The argument template's own defaults are not consulted for the parameter.
template <class X, class Y = double> struct E {};
static_assert(__is_same(U<E>::type, E<char, int>));

// Explicit arguments still win over the parameter's default.
template <template <class, class = int> class C> struct V { using type = C<char, bool>; };
static_assert(__is_same(V<B>::type, B<char, bool>));

int main() {
  S<A> s{};
  return s.x.v[4];
}
