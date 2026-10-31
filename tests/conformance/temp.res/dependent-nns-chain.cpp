// [temp.res.general]/3, [temp.dep.type]/6, [temp.alias]/2: a
// nested-name-specifier with a dependent prefix (`X::type::`,
// `T::template M<U>::template N<V>::`) stays dependent when it is
// substituted with dependent arguments (an alias template named inside
// another template) and is resolved once the arguments are known. The
// libc++ `_ITER_CONCEPT` pattern (`__iterator/concepts.h`).
//@ expect: run
//@ stdout: 1 8 8 4 | 1 2 | 3
extern "C" int printf(const char*, ...);

template <class... T> struct Or { template <class I> using Apply = I*; };
template <class I> struct Cache { using type = Or<I, int>; };
template <class I> using IC = typename Cache<I>::type::template Apply<I>;
template <class I> concept input = requires { typename IC<I>; };
template <class I> concept fwd = input<I> && sizeof(IC<I>) == sizeof(void*);

template <class T> struct W { using type = IC<T>; };

template <class T> struct M {
  template <class U> struct N {
    template <class V> struct O { using type = V; };
  };
};
template <class T, class U, class V>
using Deep = typename M<T>::template N<U>::template O<V>::type;
template <class T> struct Z { using type = Deep<T, T, long>; };

template <class X> struct Q { using type = typename X::type::inner; };
struct In { struct type { using inner = int; }; };
template <class T> struct R { using t = typename Q<T>::type; };

// A member of the class template's pattern named through an alias
// template whose argument is dependent (libc++ `pair::_CheckArgsDep`).
template <bool B, class T, class F> struct cond { using type = T; };
template <class T, class F> struct cond<false, T, F> { using type = F; };
struct Fail {};
template <class T1> struct P {
  struct Check {
    template <int&...> static constexpr bool en() { return sizeof(T1) > 1; }
    template <class U> static constexpr bool is() { return sizeof(U) == sizeof(T1); }
  };
  template <bool M_> using Dep = typename cond<M_, Check, Fail>::type;
  int v;
  template <bool D = true, int = Dep<D>::en()> P() : v(1) {}
  template <bool D = true, int = Dep<D>::template is<T1>() ? 1 : 0> P(int) : v(2) {}
};

// An unnamed non-type template parameter pack.
template <int...> constexpr int count = 3;

int main() {
  static_assert(fwd<int>);
  P<int> a;
  P<int> b(0);
  printf("%d %d %d %d | %d %d | %d\n", fwd<int> ? 1 : 0, (int)sizeof(W<char>::type),
         (int)sizeof(Z<int>::type), (int)sizeof(R<In>::t), a.v, b.v, count<>);
}
