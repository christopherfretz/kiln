// The shape of <ext/type_traits.h> `__promote`: a class template whose
// non-type parameter defaults to a member of another class template,
// with explicit specializations that take the default.
#ifndef NTTP_DEFAULT_SPEC_H
#define NTTP_DEFAULT_SPEC_H
namespace nds {
struct true_type {};
struct false_type {};
template <typename T> struct is_int {
  enum { value = 0 };
  typedef false_type type;
};
template <> struct is_int<int> {
  enum { value = 1 };
  typedef true_type type;
};
template <typename T, bool = is_int<T>::value> struct promote {
  typedef double type;
};
template <typename T> struct promote<T, false> {};
template <> struct promote<long double> {
  typedef long double type;
};
template <> struct promote<double> {
  typedef double type;
};
template <> struct promote<float> {
  typedef float type;
};
template <typename T, int N = sizeof(T) * 2> struct twice {
  static constexpr int value = N;
};
} // namespace nds
#endif
