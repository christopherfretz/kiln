// [expr.new], [expr.delete] (P2719, WP-1088): type-aware allocation for
// array new- and delete-expressions looks in the element class. Ported
// from clang's SemaCXX/type-aware-new-delete-arrays.cpp (kiln has no
// -Wmismatched-new-delete yet: those expectations are dropped).
//@ verify
//@ requires: clang>=22
//@ args: -ferror-limit=0 -Wno-ext-cxx-type-aware-allocators -Wno-mismatched-new-delete
namespace std {
  template <class T> struct type_identity {};
  enum class align_val_t : __SIZE_TYPE__ {};
  struct destroying_delete_t { explicit destroying_delete_t() = default; };
}

using size_t = __SIZE_TYPE__;

struct BasicTypeAwareArrayAllocator {
   template <typename T> void *operator new[](std::type_identity<T>, size_t, std::align_val_t) = delete; // #1
   template <typename T> void  operator delete[](std::type_identity<T>, void*, size_t, std::align_val_t) = delete; // #2
};
void *operator new[](std::type_identity<BasicTypeAwareArrayAllocator>, size_t, std::align_val_t);
void  operator delete[](std::type_identity<BasicTypeAwareArrayAllocator>, void*, size_t, std::align_val_t);

struct BasicTypeAwareNonArrayAllocator {
   template <typename T> void *operator new[](std::type_identity<T>, size_t, std::align_val_t);
   template <typename T> void  operator delete[](std::type_identity<T>, void*, size_t, std::align_val_t);
   void *operator new(size_t) = delete;
   void operator delete(void*) = delete;
};

struct WorkingTypeAwareAllocator {
   template <typename T> void *operator new[](std::type_identity<T>, size_t, std::align_val_t);
   template <typename T> void  operator delete[](std::type_identity<T>, void*, size_t, std::align_val_t);
};

void *operator new[](std::type_identity<WorkingTypeAwareAllocator>, size_t, std::align_val_t) = delete;
void  operator delete[](std::type_identity<WorkingTypeAwareAllocator>, void*, size_t, std::align_val_t) = delete;


void test() {
  BasicTypeAwareArrayAllocator *A0 = new BasicTypeAwareArrayAllocator[10];
  // expected-error@-1 {{call to deleted function 'operator new[]'}}
  // expected-note@#1 {{candidate function [with T = BasicTypeAwareArrayAllocator] has been explicitly deleted}}
  delete [] A0;
  // expected-error@-1 {{attempt to use a deleted function}}
  // expected-note@#2 {{'operator delete[]<BasicTypeAwareArrayAllocator>' has been explicitly marked deleted here}}
  
  BasicTypeAwareNonArrayAllocator *A1 = new BasicTypeAwareNonArrayAllocator[10];
  delete [] A1;

  WorkingTypeAwareAllocator *A2 = new WorkingTypeAwareAllocator[10];
  delete A2;

  WorkingTypeAwareAllocator *A3 = new WorkingTypeAwareAllocator;
  delete [] A3;

}
