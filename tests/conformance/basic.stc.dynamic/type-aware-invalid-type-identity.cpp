// [basic.stc.dynamic.allocation] (P2719, WP-1088): a std::type_identity
// that is not a class template still forms the type-aware parameter rules.
// Ported from clang's SemaCXX/type-aware-new-invalid-type-identity.cpp.
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
namespace std {
#if !defined(INVALID_TYPE_IDENTITY_VERSION)
  // expected-no-diagnostics
  template <class T> struct type_identity {
  };
  #define TYPE_IDENTITY(T) std::type_identity<T>
#elif INVALID_TYPE_IDENTITY_VERSION==0
  struct type_identity {};
  // expected-error@-1 {{std::type_identity must be a class template with a single type parameter}}
  #define TYPE_IDENTITY(T) std::type_identity
#elif INVALID_TYPE_IDENTITY_VERSION==1
  template <class A, class B> struct type_identity {};
  // expected-error@-1 {{std::type_identity must be a class template with a single type parameter}}
  #define TYPE_IDENTITY(T) std::type_identity<T, int>
#elif INVALID_TYPE_IDENTITY_VERSION==2
  enum type_identity {};
  // expected-error@-1 {{std::type_identity must be a class template with a single type parameter}}
  #define TYPE_IDENTITY(T) std::type_identity
#elif INVALID_TYPE_IDENTITY_VERSION==3
  template <class T> using type_identity = int;
  #define TYPE_IDENTITY(T) std::type_identity<T>
#elif INVALID_TYPE_IDENTITY_VERSION==4
  template <class T> struct inner {};
  template <class T> using type_identity = inner<T>;
  #define TYPE_IDENTITY(T) std::type_identity<T>
#endif
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : long {};
}

template <class T> void *operator new(TYPE_IDENTITY(T), std::size_t, std::align_val_t); // #operator_new
template <class T> void operator delete(TYPE_IDENTITY(T), void*, std::size_t, std::align_val_t); // #operator_delete

// These error messages aren't great, but they fall out of the way we model
// alias types. Getting them in this way requires extremely unlikely code to be
// used, so this is not terrible.

#if INVALID_TYPE_IDENTITY_VERSION==3
// expected-error@#operator_new {{'operator new' takes type size_t ('unsigned long') as 1st parameter}}
// expected-error@#operator_delete {{1st parameter of 'operator delete' must have type 'void *'}}
#elif INVALID_TYPE_IDENTITY_VERSION==4
// expected-error@#operator_new {{'operator new' cannot take a dependent type as its 1st parameter; use size_t ('unsigned long') instead}}
// expected-error@#operator_delete {{'operator delete' cannot take a dependent type as its 1st parameter; use 'void *' instead}}
#endif

using size_t = __SIZE_TYPE__;
struct TestType {};

void f() {
  TestType *t = new TestType;
  delete t;
}
