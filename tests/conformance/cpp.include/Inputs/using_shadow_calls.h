// Function templates whose bodies call names found through
// using-declarations (WP-1342): an overload set, a function template
// with explicit arguments, and one named in a class template's member.
#ifndef USING_SHADOW_CALLS_H
#define USING_SHADOW_CALLS_H
namespace usc_lib {
constexpr int mag(int x) { return x < 0 ? -x : x; }
constexpr long mag(long x) { return x < 0 ? -x : x; }
template <class T> constexpr T twice(T t) { return t + t; }
} // namespace usc_lib
namespace usc {
using usc_lib::mag;
using usc_lib::twice;
template <class T> constexpr T alt(T t) { return mag(t) + twice(t); }
template <class T> struct holder {
  static T go(T t) { return twice<T>(mag(t)); }
};
} // namespace usc
#endif
