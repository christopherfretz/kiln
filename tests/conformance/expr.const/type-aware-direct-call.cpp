// [expr.const]/10.18 (P2719, WP-1112): a direct call of a global
// type-aware allocation function is, like `::operator new(n)`, constant
// only within `std::allocator<T>::allocate` (`deallocate`): elsewhere
// `operator new` is "untyped memory" and `operator delete` not valid
// (clang `isUsableAsGlobalAllocationFunctionInConstantEvaluation`,
// `HandleOperatorNewCall` / `HandleOperatorDeleteCall`). A class-scope
// one is an ordinary non-constexpr function.
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
namespace std {
  template <class T> struct type_identity {};
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : size_t {};
}
using size_t = __SIZE_TYPE__;
template <class T> void *operator new(std::type_identity<T>, size_t, std::align_val_t);
template <class T> void operator delete(std::type_identity<T>, void *, size_t, std::align_val_t);
struct S { int x; };

constexpr int direct_new() {
  void *p = ::operator new(std::type_identity<S>{}, sizeof(S), std::align_val_t{alignof(S)}); // expected-note {{cannot allocate untyped memory in a constant expression; use 'std::allocator<T>::allocate' to allocate memory of type 'T'}}
  return p != nullptr;
}
static_assert(direct_new()); // expected-error {{static assertion expression is not an integral constant expression}}
// expected-note@-1 {{in call to 'direct_new()'}}

constexpr int direct_delete() {
  S *p = new S{1};
  ::operator delete(std::type_identity<S>{}, p, sizeof(S), std::align_val_t{alignof(S)}); // expected-note {{subexpression not valid in a constant expression}}
  return 1;
}
static_assert(direct_delete()); // expected-error {{static assertion expression is not an integral constant expression}}
// expected-note@-1 {{in call to 'direct_delete()'}}

struct C {
  int x;
  void *operator new(std::type_identity<C>, size_t, std::align_val_t); // #cnew
  void operator delete(std::type_identity<C>, void *, size_t, std::align_val_t);
};
constexpr int member_new() {
  void *p = C::operator new(std::type_identity<C>{}, sizeof(C), std::align_val_t{alignof(C)}); // expected-note {{non-constexpr function 'operator new' cannot be used in a constant expression}}
  return p != nullptr;
}
static_assert(member_new()); // expected-error {{static assertion expression is not an integral constant expression}}
// expected-note@-1 {{in call to 'member_new()'}}
// expected-note@#cnew {{declared here}}
