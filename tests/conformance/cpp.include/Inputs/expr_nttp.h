#ifndef EXPR_NTTP_H
#define EXPR_NTTP_H
template <class T, T v> struct ic {
  static constexpr T value = v;
  using type = ic;
  constexpr operator T() const { return v; }
};
template <bool B> using bc = ic<bool, B>;
template <class T> struct is_ptr : ic<bool, false> {};
template <class T> struct is_ptr<T *> : ic<bool, true> {};
template <class T> using not_ptr = bc<!is_ptr<T>::value>;
template <class T> using size_of = ic<unsigned long, sizeof(T)>;
template <int N> using twice = ic<int, N * 2>;
using three = ic<int, 1 + 2>;
template <class T> struct holder { ic<int, sizeof(T) + 1> extra; };
#endif
