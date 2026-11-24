// Function-name spans (spec/ast.md §24.4): a conversion function's name
// is one entry over `operator <type>`, the target's names one depth in;
// in a conversion function template the target is the substituted one
// and the return type the pattern's.
//@ fn-names: names-conversions.names
//@ std: c++20
namespace ns {
struct S {};
template <class T> struct W {};
struct K {
  operator int() { (void)__PRETTY_FUNCTION__; return 0; }
  operator S() { (void)__PRETTY_FUNCTION__; (void)__func__; return {}; }
  operator const S *() { (void)__PRETTY_FUNCTION__; return nullptr; }
  operator W<S>() { (void)__PRETTY_FUNCTION__; return {}; }
  template <class T> operator T *() { (void)__PRETTY_FUNCTION__; return nullptr; }
};
}
void use() {
  ns::K k;
  (void)int(k);
  (void)static_cast<ns::S>(k);
  (void)static_cast<const ns::S *>(k);
  (void)static_cast<ns::W<ns::S>>(k);
  (void)static_cast<ns::S *>(k);
}
