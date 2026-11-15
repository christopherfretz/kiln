// C++14 [except.spec]: with -fno-exceptions nothing can unwind, so a
// dynamic exception specification emits no filter, no landing pad and no
// `__cxa_call_unexpected`; the functions are `nounwind` as clang emits
// them (companion of ir-dynamic-spec-filter-cxx14.cpp).
//@ expect: ir
//@ min-oracle: 22
//@ std: c++14
//@ args: -fno-exceptions
struct D { ~D(); };
void g();
void f1() throw(int) { D d; g(); }
void f2() throw() { g(); }
