// [basic.stc.dynamic.general]/2, [dcl.link] (P2719, WP-1088): a type-aware
// allocation function declared in a linkage-specification still belongs to
// the global scope, and new/delete-expressions use it. After clang's
// SemaCXX/type-aware-new-delete-transparent-contexts.cpp.
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators
// expected-no-diagnostics
namespace std {
  template <class T> struct type_identity {};
  using size_t = __SIZE_TYPE__;
  enum class align_val_t : size_t {};
}

extern "C" {
  void *operator new(std::type_identity<int>, std::size_t, std::align_val_t);
  void operator delete[](std::type_identity<int>, void*, std::size_t, std::align_val_t);
}
extern "C++" {
  void *operator new(std::type_identity<long>, std::size_t, std::align_val_t);
  void operator delete[](std::type_identity<long>, void*, std::size_t, std::align_val_t);
}

void *operator new[](std::type_identity<int>, std::size_t, std::align_val_t);
void operator delete(std::type_identity<int>, void*, std::size_t, std::align_val_t);
void *operator new[](std::type_identity<long>, std::size_t, std::align_val_t);
void operator delete(std::type_identity<long>, void*, std::size_t, std::align_val_t);

void foo() {
  int *iptr = new int;
  delete iptr;
  int *iarray = new int[5];
  delete [] iarray;
  long *lptr = new long;
  delete lptr;
  long *larray = new long[5];
  delete [] larray;
}
