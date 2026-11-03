// [expr.new]/28: without exceptions no deallocation function is looked up
// for a new-expression (clang `FindAllocationFunctions` skips it under
// -fno-exceptions), so an inaccessible one is not diagnosed.
//@ args: -fno-exceptions
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:19 error "'operator new' is a private member of 'A'"
//@ diag: 9:27 note "declared private here"
struct A { private: void *operator new(__SIZE_TYPE__); void operator delete(void *); };
void f() { A *a = new A; (void)a; }
struct C { protected: void operator delete(void *); public: C(); };
C *g() { return new C; }
