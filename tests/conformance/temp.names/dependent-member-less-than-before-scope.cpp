// [temp.names]/3: a `<` after a name that is not known to be a template
// is the less-than operator. In `if_c<rank<T0>::value < rank<T1>::value,
// T1, T0>::type` the member `value` of the dependent `rank<T0>` is not
// a template, so the `<` compares: the tokens up to `>::type` are not a
// template argument list for `value`, even though one would close there.
// Clang reads such a `<` as a list (and asks for `template`) only when
// nothing else fits: `<>` or a leading unambiguous type
// (Boost.Spirit classic phoenix `higher_rank`, WP-804).
//@ expect: run
//@ strict-diags
//@ stdout: 4 1 8

#include <cstdio>

template <bool B, class T, class F> struct if_c { typedef T type; };
template <class T, class F> struct if_c<false, T, F> { typedef F type; };
template <class T> struct rank { static const int value = sizeof(T); };

template <class T0, class T1> struct higher_rank {
  typedef typename if_c<rank<T0>::value < rank<T1>::value, T1, T0>::type type;
};

template <class T> struct lower {
  static const bool small = rank<T>::value < 4;
  typedef typename if_c<rank<T>::value < 4, char, double>::type type;
};

int main() {
  std::printf("%d %d %d\n", (int)sizeof(higher_rank<int, char>::type),
              (int)sizeof(higher_rank<char, char>::type), (int)sizeof(lower<long>::type) + lower<long>::small);
}
