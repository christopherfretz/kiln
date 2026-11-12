// The shape of <bits/basic_ios.tcc>: a class template named (and so
// declared as a specialization) in one namespace block, and explicitly
// instantiated in a later block of the same namespace.
#ifndef REOPENED_EXTERN_TEMPLATE_H
#define REOPENED_EXTERN_TEMPLATE_H
namespace ret {
template <class T> struct basic {
  T v;
  int width() const { return int(sizeof(T)) * 10 + int(v); }
};
typedef basic<char> narrow;
typedef basic<long> wide;
} // namespace ret

namespace ret {
template <class T> int twice(const basic<T> &b) { return 2 * b.width(); }
extern template struct basic<char>;
template struct basic<long>;
} // namespace ret
#endif
