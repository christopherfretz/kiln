// [temp.names]/3, [basic.lookup.qual.general]/1, [temp.expl.spec]/1:
// in `X<A>::apply<...>` with the qualifier not dependent, lookup of
// `apply` is in the specialization `X<A>` names, which finds a member
// template, so `<` opens a template argument list without the `template`
// keyword. That holds when the primary template `X` is only declared and
// `X<A>` is an explicit (or partial) specialization, and when the
// template-id stands inside another template argument list or a base
// clause of a template. boost.mpl's map/aux_/contains_impl.hpp:36,
// has_key_impl.hpp:34 and set/aux_/at_impl.hpp:30
// (`at_impl<aux::map_tag>::apply<Map, typename Pair::first>::type`,
// `if_<has_key_impl<aux::set_tag>::apply<Set, T>, ...>`).
// Verified against clang 21.
//@ expect: run
//@ strict-diags
//@ stdout: 1 0 2 8 4
#include <stdio.h>

template <class T, class U> struct is_same { static const bool value = false; };
template <class T> struct is_same<T, T> { static const bool value = true; };
template <bool C, class T, class F> struct select { typedef T type; };
template <class T, class F> struct select<false, T, F> { typedef F type; };
template <class C, class T, class F> struct if_ : select<C::value, T, F> {};
template <class T> struct wrapped { typedef T type; };

struct map_tag;
struct set_tag;

// Primary templates only declared; each tag's explicit specialization
// declares the member template `apply`.
template <class Tag> struct at_impl;
template <class Tag> struct has_key_impl;
template <class Tag> struct contains_impl;

template <class M, class K> struct m_at { typedef K type; };
template <> struct at_impl<map_tag> {
  template <class M, class K> struct apply : wrapped<typename m_at<M, K>::type> {};
};

// The template-id inside a template argument of a base class.
template <> struct contains_impl<map_tag> {
  template <class Map, class Pair>
  struct apply
      : is_same<typename at_impl<map_tag>::apply<Map, typename Pair::first>::type,
                typename Pair::second> {};
};

template <> struct has_key_impl<set_tag> {
  template <class S, class T> struct apply { static const bool value = false; };
};

// The template-id as a template argument of a typedef's type.
template <> struct at_impl<set_tag> {
  template <class Set, class T> struct apply {
    typedef typename if_<has_key_impl<set_tag>::apply<Set, T>, T, short>::type type;
  };
};

// A partial specialization of a primary template that is only declared.
template <class T> struct ptr_impl;
template <class T> struct ptr_impl<T *> {
  template <class U> struct apply { typedef U type[2]; };
};
template <class T> struct uses_ptr {
  typedef typename wrapped<ptr_impl<int *>::apply<T> >::type::type type;
};

struct same_pair { typedef int first; typedef int second; };
struct other_pair { typedef int first; typedef long second; };

int main() {
  printf("%d %d %d %d %d\n", (int)contains_impl<map_tag>::apply<void, same_pair>::value,
         (int)contains_impl<map_tag>::apply<void, other_pair>::value,
         (int)sizeof(at_impl<set_tag>::apply<int, int>::type),
         (int)sizeof(uses_ptr<int>::type), (int)sizeof(uses_ptr<short>::type));
}
