// The interface of module_explicit_inst_importer.cpp: one template of each
// kind, with explicit instantiation declarations (double, float,
// unsigned) and definitions (long) of some specializations.
export module mod_explinst;
export {
template <class T> T twice(T t) { return t + t; }
template <class T> struct S {
  T get(T t) { return t + 1; }
};
template <class T> struct V {
  static T v;
};
template <class T> T vt = T(5);
template <class T> struct C {
  T f(T t) { return t * 2; }
  static T s;
};
template <class T> struct D {
  virtual T f(T t) { return t * 3; }
};
}
template <class T> T V<T>::v = T(7);
template <class T> T C<T>::s = T(3);

extern template double twice<double>(double);
extern template double S<double>::get(double);
extern template double vt<double>;
extern template struct D<double>;
extern template float twice<float>(float);
extern template float S<float>::get(float);
extern template float V<float>::v;
extern template float vt<float>;
extern template struct C<float>;
extern template struct D<float>;
extern template unsigned twice<unsigned>(unsigned);
extern template unsigned S<unsigned>::get(unsigned);
extern template unsigned V<unsigned>::v;
extern template unsigned vt<unsigned>;
extern template struct C<unsigned>;
extern template struct D<unsigned>;

template long twice<long>(long);
template long S<long>::get(long);
template long V<long>::v;
template long vt<long>;
template struct C<long>;
template struct D<long>;
