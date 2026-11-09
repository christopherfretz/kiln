// [basic.stc.dynamic.allocation], [expr.new]/19, [expr.delete]/10 (P2719,
// WP-1112): a std::type_identity template with a parameter pack is not a class template with a
// single type parameter; the type-aware functions declared with it are
// unusable, each new-expression (and each class-scope delete-expression)
// is diagnosed at the declaration (clang `LookupStdClassTemplate`), and
// a new-expression falls back to the untyped functions.
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
namespace std {
  template <class... T> struct type_identity {}; // #ti
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : long {};
}
#define TI(T) std::type_identity<T>

template <class T> void *operator new(TI(T), std::size_t, std::align_val_t);
template <class T> void operator delete(TI(T), void *, std::size_t, std::align_val_t);

struct T {};
void global() {
  T *p = new T;
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
  delete p;
}

struct S {
  template <class U> void *operator new(TI(U), std::size_t, std::align_val_t);
  template <class U> void operator delete(TI(U), void *, std::size_t, std::align_val_t); // #sdel
};
void member(S *p) {
  delete p; // expected-error {{no suitable member 'operator delete' in 'S'}}
  // expected-note@#sdel {{member 'operator delete' declared here}}
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
  new S; // expected-error {{no matching function for call to 'operator new'}}
  // expected-error@#ti {{std::type_identity must be a class template with a single type parameter}}
}
