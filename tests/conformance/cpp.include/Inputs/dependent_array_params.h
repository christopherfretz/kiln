// WP-1320: function template parameters of a dependent array type, whose
// adjusted types name the canonical template parameter type.
#ifndef DEPENDENT_ARRAY_PARAMS_H
#define DEPENDENT_ARRAY_PARAMS_H
namespace dap {
template <class T> T first2(const T a[2]) { return a[0] + a[1]; }
template <class T> T head(T a[]) { return a[0]; }
template <class T> struct holder {
  T v;
  template <class U> T scaled(const U k[3]) const { return v * T(k[2]); }
};
} // namespace dap
#endif
