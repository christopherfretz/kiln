#ifndef MEMBER_ENUMS_H
#define MEMBER_ENUMS_H
// The shape of <__type_traits/is_integral.h>: enumerations in class
// templates, their enumerators' values set by each instantiation.
namespace me {
template <class T> struct A {
  enum { a = 1, b, c = sizeof(T) * 2, d = c + b };
  enum E : long { x = a + 10, y };
  enum class S { p = sizeof(T), q };
  int get() const { return d + int(S::q); }
};
template <class T> struct is_int {
  enum { value = 0 };
};
template <> struct is_int<int> {
  enum { value = 1 };
};
template <class T> struct B {
  enum { v = T::value + 1 };
};
} // namespace me
#endif
