// [temp.inst]/7, [class.static.data]/3: a member variable template of a
// class (or class template) defined out of line. Its implicit
// specializations take their initializer from the out-of-line definition,
// so they are not "initialized in the class" and are emitted as
// linkonce_odr definitions; in-class const, inline and constexpr member
// variable templates keep their usual treatment.
struct A {
  template <class T> static T v;
  template <class T> static const T c = T(2);
  template <class T> static inline T i = T(5);
  template <class T> static constexpr T k = T(6);
};
template <class T> T A::v = T(1);

template <class T> struct B {
  template <class U> static U w;
  template <class U> static const U c = U(3);
  template <class U> static inline U i = U(7);
};
template <class T> template <class U> U B<T>::w = U(4);

int use_v() { return A::v<int> + A::c<int> + B<char>::w<int> + B<int>::w<long>; }
const int *p() { return &A::c<int>; }
const long *q() { return &B<char>::c<long>; }
int r() { return A::i<int> + B<int>::i<short> + *&A::k<int>; }
