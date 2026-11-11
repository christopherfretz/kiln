#ifndef EXPR_DECLTYPE_H
#define EXPR_DECLTYPE_H
inline int dt_g = 4;
inline decltype(dt_g + 1L) dt_long() { return dt_g + 1L; }
template <class T> T dt_val();
template <class T, class U> decltype(T() + U()) dt_add(T t, U u) { return t + u; }
template <class T> struct dt_box { using type = decltype(dt_val<T>() * 2); };
template <class T> using dt_t = decltype(T() - 1);
template <class T> auto dt_has_size(int) -> decltype(sizeof(typename T::size_type), true) {
  return true;
}
template <class T> bool dt_has_size(...) { return false; }
#endif
