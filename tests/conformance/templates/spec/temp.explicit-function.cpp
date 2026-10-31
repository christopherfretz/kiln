// [temp.explicit]: explicit instantiation definitions and declarations
// of function templates, deduced and with explicit arguments, and of
// members of class templates.
//@ expect: compile-ok
template <class T> T twice(T t) { return t + t; }
template int twice(int);
template double twice<double>(double);
extern template long twice(long);
template <class T> struct A {
  int f() { return sizeof(T); }
  template <class U> U g(U u) { return u; }
};
template int A<char>::f();
extern template int A<short>::f();
template int A<int>::g(int);
namespace q { template <class T> void w(T) {} }
template void q::w(char);
template void q::w<>(short);
