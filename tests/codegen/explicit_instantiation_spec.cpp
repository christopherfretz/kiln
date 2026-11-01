// [temp.explicit]/10: an explicit instantiation definition of a class
// does not instantiate members that were explicitly specialized before
// it; those are ordinary (strong) definitions emitted where they appear
// (clang `InstantiateClassMembers` skips `TSK_ExplicitSpecialization`).
// [temp.expl.spec]/1: an out-of-line `template<>` redeclaration of a
// static data member, constructor or destructor of an implicit
// specialization is an explicit specialization (strong, with C1/D1
// aliases), with or without a later explicit instantiation.
template <class T> struct X {
  void spec();
  void ispec() {}
  static T s;
  struct N { void n() {} };
  template <class U> void mt() {}
  X();
  ~X();
};
template <class T> void X<T>::spec() {}
template <class T> T X<T>::s = 1;
template <class T> X<T>::X() {}
template <class T> X<T>::~X() {}
template <> void X<int>::spec() {}
template <> void X<int>::ispec() {}
template <> int X<int>::s = 5;
template <> X<int>::X() {}
template <> X<int>::~X() {}
template struct X<int>;

template <class T> struct Y {
  static T s;
  Y();
  ~Y();
};
template <class T> T Y<T>::s = 1;
template <class T> Y<T>::Y() {}
template <class T> Y<T>::~Y() {}
template <> long Y<long>::s = 5;
template <> Y<long>::Y() {}
template <> Y<long>::~Y() {}
long use() { Y<long> y; return Y<long>::s; }
