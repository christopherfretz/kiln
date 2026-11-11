#ifndef EXPR_ALIAS_BASE_H
#define EXPR_ALIAS_BASE_H
template <class T, T v> struct ic {
  static constexpr T value = v;
};
template <bool B> using bc = ic<bool, B>;
template <class T> struct is_ptr : ic<bool, false> {};
template <class T> struct is_ptr<T *> : ic<bool, true> {};
template <class T> struct not_ptr : bc<!is_ptr<T>::value> {};
template <class T> struct wide : ic<unsigned long, sizeof(T) * 2> {};
#endif
