// [basic.stc.dynamic.allocation], [expr.new]/19 (P2719, WP-1112): a
// std::type_identity template with a single required type parameter
// (a later one defaulted) is usable (clang `LookupStdClassTemplate`:
// `getMinRequiredArguments() == 1`).
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
// expected-no-diagnostics
namespace std {
  template <class T, class U = int> struct type_identity {};
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : long {};
}

struct S {
  template <class T> void *operator new(std::type_identity<T>, std::size_t, std::align_val_t);
  template <class T> void operator delete(std::type_identity<T>, void *, std::size_t, std::align_val_t);
};
void f(S *p) {
  delete p;
  new S;
}
