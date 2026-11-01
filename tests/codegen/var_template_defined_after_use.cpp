// [temp.inst]/7, [temp.point]/7: an implicit variable template
// specialization odr-used before its template is defined is instantiated at
// the end of the translation unit from the definition seen since, and emitted
// as a linkonce_odr definition like any other implicit specialization (clang
// `InstantiateVariableDefinition` from `PerformPendingInstantiations`).
template <class T> extern T v;
int f() { return v<int> + v<long>; }
template <class T> T v = T(3);

struct M {
  template <class T> static T s;
};
int k() { return M::s<int>; }
template <class T> T M::s = T(7);

template <class T> struct B {
  template <class U> static U w;
};
long *q() { return &B<char>::w<long>; }
template <class T> template <class U> U B<T>::w = U(4);

// One whose initializer runs at startup.
int init();
template <class T> extern T d;
int h() { return d<int>; }
template <class T> T d = T(init());
