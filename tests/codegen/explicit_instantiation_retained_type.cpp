// An explicit instantiation definition of a class template keeps the
// specialization's type, complete, in the unit's retained types, though no
// member of it is emitted (clang `completeTemplateDefinition`). A member class
// of the specialization is not retained by it.
//@ args: -g
template <class T> struct P {
  T a;
};
template struct P<int>;
template <class T> struct Q {
  struct In {
    T b;
  };
  void f() {}
  In c;
};
template struct Q<long>;
