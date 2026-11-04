//@ args: -std=c++26
// Template visibility (spec/codegen.md "Symbol visibility"; clang
// mergeTemplateLV): a specialization's visibility merges the template's
// with that of its template arguments (types, and the type of a
// class-type non-type argument's template parameter object); the most
// restrictive wins, unless the specialization or its explicit
// instantiation carries an explicit attribute. The template's own
// attribute is not copied onto its specializations.
struct __attribute__((visibility("hidden"))) H {};
struct D {};
template <class T> void tf() {}
template void tf<H>();
template void tf<D>();
template <class T> __attribute__((visibility("default"))) void dtf() {}
template void dtf<int>();
template void dtf<H>();
template <class T> struct __attribute__((visibility("default"))) TD {
  void f() {}
  static int s;
};
template <class T> int TD<T>::s = 0;
template struct TD<H>;
template struct TD<int>;
template <class T> struct TP {
  virtual void v() {}
};
template struct TP<H *>;
template struct TP<D[2]>;
struct P {
  int x;
};
template <P p> const P *addr() { return &p; }
struct __attribute__((visibility("hidden"))) HP {
  int x;
};
template <HP p> const HP *addrh() { return &p; }
int use() {
  return addr<P{1}>()->x + addrh<HP{1}>()->x + TD<H>::s + TD<int>::s;
}
