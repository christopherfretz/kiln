// [expr.new]/28 (P3769R1, a DR): the selected placement deallocation
// function must be non-deleted and accessible from the new-expression;
// a usual deallocation function with a `std::size_t` parameter selected
// as a placement one makes the program ill-formed (C++14 sized
// deallocation), unless a class-scope unsized one corresponds to it.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:3 error "attempt to use a deleted function"
//@ diag: 17:6 note "'operator delete' has been explicitly marked deleted here"
//@ diag: 40:3 error "'operator delete' is a private member of 'P'"
//@ diag: 24:15 note "declared private here"
//@ diag: 41:3 error "'new' expression with placement arguments refers to non-placement 'operator delete'"
//@ diag: 29:15 note "'operator delete' declared here"
#include <cstddef>
struct A {};
void* operator new(std::size_t, A&);
void operator delete(void*, A&) = delete;
struct T { T(); };
struct P {
  P();
  static void* operator new(std::size_t, int);
  static void operator delete(void*);
private:
  static void operator delete(void*, int);
};
struct S {
  S();
  static void* operator new(std::size_t, std::size_t);
  static void operator delete(void*, std::size_t);
};
// Not really sized: the unsized usual member corresponds to it.
struct U {
  U();
  static void* operator new(std::size_t, std::size_t);
  static void operator delete(void*, std::size_t);
  static void operator delete(void*);
};
void f(A& a) {
  new (a) T;
  new (1) P;
  new (1) S;
  new (1) U;
}
