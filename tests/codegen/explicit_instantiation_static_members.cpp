// [temp.explicit]/10, /11, [basic.start.dynamic]/1: explicit instantiation
// definitions of a class template, a member class, a static data member
// and a variable template emit their dynamic initializers as unordered
// (`llvm.global_ctors` entries keyed by their comdat); `extern template`
// keeps suppressing emission for X<int>.
int side();
struct D { D(); ~D(); int v; };
template <class T> struct X {
  static D sd;
  static T st;
  static inline D si{};
  virtual int vf() { return 3; }
  struct In { static int n; int g() { return n; } };
  X() : m(1) {}
  int m;
};
template <class T> D X<T>::sd;
template <class T> T X<T>::st = side();
template <class T> int X<T>::In::n = side();
template struct X<long>;
template <class T> T vt = T(side());
template long vt<long>;
template int X<short>::st;
template struct X<short>::In;
extern template struct X<int>;
int use() { return X<int>::st + vt<long>; }
