// [expr.new]/28, [class.access]/4: the deallocation function a
// new-expression would call if the initialization threw is looked up and
// access-checked at the new-expression, whether or not the constructor
// can throw; an inaccessible allocation function does not stop it from
// being checked (clang `FindAllocationFunctions` → `CheckAllocationAccess`
// for both).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:19 error "'operator new' is a private member of 'A'"
//@ diag: 15:27 note "declared private here"
//@ diag: 18:19 error "'operator delete' is a private member of 'A'"
//@ diag: 15:61 note "declared private here"
//@ diag: 19:19 error "'operator delete' is a protected member of 'C'"
//@ diag: 16:28 note "declared protected here"
struct A { private: void *operator new(__SIZE_TYPE__); void operator delete(void *); };
struct C { protected: void operator delete(void *); public: void *operator new(__SIZE_TYPE__); C(); };
struct D : C { static D *make() { return new D; } };
void f() { A *a = new A; (void)a; }
void g() { C *c = new C; (void)c; }
