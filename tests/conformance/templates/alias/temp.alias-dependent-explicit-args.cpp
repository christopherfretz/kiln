// [temp.dep.expr]/3, [temp.alias]/2: a call naming a function template
// with dependent explicit template arguments is type-dependent when an
// alias template is substituted with a dependent argument, so the
// function's definition is not instantiated (for its deduced return
// type) until the argument is known (libc++ ranges_iterator_concept.h
// `__iterator_concept` used from `_IterOps`'s member alias).
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

template <class T> concept big = sizeof(T) > 4;
namespace r {
  template <class T> constexpr auto g() {
    if constexpr (big<T>)
      return 1L;
    else
      return 1;
  }
  template <class T> using G = decltype(g<T>());
}

template <class P> struct Ops { template <class I> using cat = r::G<I>; };
static_assert(same<Ops<int>::cat<long>, long>::value);
static_assert(same<Ops<int>::cat<char>, int>::value);

template <class U> struct H { using t = r::G<U>; };
static_assert(same<H<double>::t, long>::value);
static_assert(same<H<short>::t, int>::value);

int main() {}
