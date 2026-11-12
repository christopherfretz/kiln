// A module with one template of each kind; explicit instantiation
// declarations of the double specializations, definitions of the long
// ones (explicit-inst-owner-run.cpp, explicit-inst-owner-errors.cpp,
// explicit-inst-sdm-importer-run.cpp).
export module xiown;
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
extern template double V<double>::v;
extern template double vt<double>;
extern template struct C<double>;
extern template struct D<double>;

template long twice<long>(long);
template long S<long>::get(long);
template long V<long>::v;
template long vt<long>;
template struct C<long>;
template struct D<long>;
