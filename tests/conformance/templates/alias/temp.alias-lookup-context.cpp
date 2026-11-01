// [temp.res.general]/1, [temp.dep.candidate]: a name in an alias
// template's aliased type is looked up from the alias's declaration
// context, not from where the alias is used. A call naming a function
// template with explicit template arguments in a member alias finds the
// class member when the alias is used from another class template
// (libc++ sfinae_helpers.h `__do_test`).
//@ expect: compile-ok
//@ strict-diags
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

struct B {
  template <class Tr, class T> static T f(T);
  template <class T> using ok = decltype(f<int>(T{}));
};
template <class T> struct C { B::ok<T> m; };
C<int> c;
static_assert(same<decltype(c.m), int>::value);

template <class U> struct D {
  template <class Tr, class T> static Tr g(T);
  template <class T> using ok = decltype(g<U>(T{}));
};
template <class T> struct E { typename D<long>::template ok<T> m; };
static_assert(same<decltype(E<char>{}.m), long>::value);

namespace n {
  struct tag {};
  template <class Tr, class T> Tr h(T);
  template <class T> using ok = decltype(h<short>(T{}));
}
namespace m {
  template <class T> struct F { n::ok<T> v; };
  static_assert(same<decltype(F<int>{}.v), short>::value);
}

int main() {}
