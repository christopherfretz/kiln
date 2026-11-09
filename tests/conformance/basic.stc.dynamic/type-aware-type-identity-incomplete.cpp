// [expr.new]/19, [expr.delete]/10, [temp.inst]/2 (P2719, WP-1112): the
// `std::type_identity<T>` argument of a type-aware allocation function
// must be complete; an incomplete one is diagnosed once at the start of
// the new- or delete-expression, which is then invalid (clang
// `FindAllocationFunctions` / `CheckDeleteOperator` `RequireCompleteType`).
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
namespace std {
  template <class T> struct type_identity; // #ti
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : long {};
}

template <class T> void *operator new(std::type_identity<T>, std::size_t, std::align_val_t);
template <class T> void operator delete(std::type_identity<T>, void *, std::size_t, std::align_val_t);

struct T {};
void global(T *p) {
  new T; // expected-error {{implicit instantiation of undefined template 'std::type_identity<T>'}}
  // expected-note@#ti {{template is declared here}}
  delete p; // expected-error {{implicit instantiation of undefined template 'std::type_identity<T>'}}
  // expected-note@#ti {{template is declared here}}
}

struct S {
  template <class U> void *operator new(std::type_identity<U>, std::size_t, std::align_val_t);
  template <class U> void operator delete(std::type_identity<U>, void *, std::size_t, std::align_val_t);
};
void member(S *p) {
  delete p; // expected-error {{implicit instantiation of undefined template 'std::type_identity<S>'}}
  // expected-note@#ti {{template is declared here}}
  new S; // expected-error {{implicit instantiation of undefined template 'std::type_identity<S>'}}
  // expected-note@#ti {{template is declared here}}
}
constexpr int unevaluated() { return sizeof(new S); } // expected-error {{implicit instantiation of undefined template 'std::type_identity<S>'}}
// expected-note@#ti {{template is declared here}}
