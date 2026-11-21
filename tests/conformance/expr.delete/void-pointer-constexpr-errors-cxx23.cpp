// Before C++26 (an extension, -Wdelete-incomplete), clang's constant
// evaluator deletes through a 'void *' by the allocation's dynamic type,
// with the checks of any delete-expression: the form must match the
// allocation's, the pointer must be to the whole allocation (a pointer to
// a base class subobject is one through a static type, 'void', without a
// virtual destructor), the object must be heap-allocated and still live.
// A null pointer is a no-op; each element of an array is destroyed.
//@ std: c++23
//@ args: -Wno-delete-incomplete -Wno-mismatched-new-delete
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:53 note "array delete used to delete pointer to non-array object of type 'S'"
//@ diag: 39:15 note "in call to 'h()'"
//@ diag: 38:42 note "heap allocation performed here"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:63 note "non-array delete used to delete pointer to array object of type 'S[2]'"
//@ diag: 41:15 note "in call to 'i()'"
//@ diag: 40:42 note "heap allocation performed here"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:46 note "delete of pointer '&x' that does not point to a heap-allocated object"
//@ diag: 45:15 note "in call to 'k()'"
//@ diag: 44:25 note "declared here"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:77 note "delete of pointer to subobject"
//@ diag: 47:15 note "in call to 'l()'"
//@ diag: 52:15 error "static assertion expression is not an integral constant expression"
//@ diag: 51:70 note "delete of object with dynamic type 'D' through pointer to base class type 'void' with non-virtual destructor"
//@ diag: 52:15 note "in call to 'n()'"
//@ diag: 54:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:63 note "delete of pointer that has already been deleted"
//@ diag: 54:15 note "in call to 'o()'"
struct S { int *c; constexpr ~S() { ++*c; } };
constexpr int f() { int c = 0; void *p = new S{&c}; delete p; return c; }
static_assert(f() == 1);
constexpr int g() { int c = 0; void *p = new S[3]{{&c},{&c},{&c}}; delete[] p; return c; }
static_assert(g() == 3);
constexpr int h() { int c = 0; void *p = new S{&c}; delete[] p; return c; }
static_assert(h() == 1);
constexpr int i() { int c = 0; void *p = new S[2]{{&c},{&c}}; delete p; return c; }
static_assert(i() == 2);
constexpr int j() { void *p = nullptr; delete p; return 1; }
static_assert(j() == 1);
constexpr int k() { int x = 0; void *p = &x; delete p; return 1; }
static_assert(k() == 1);
constexpr int l() { int c = 0; S *s = new S[2]{{&c},{&c}}; void *p = s + 1; delete[] p; return c; }
static_assert(l() == 2);
constexpr int m() { int c = 0; const void *p = new const S{&c}; delete p; return c; }
static_assert(m() == 1);
struct B { virtual constexpr ~B() {} }; struct D : B { int *c; constexpr D(int*c):c(c){} constexpr ~D() { ++*c; } };
constexpr int n() { int c = 0; void *p = static_cast<B*>(new D(&c)); delete p; return c; }
static_assert(n() == 1);
constexpr int o() { int c = 0; void *p = new S{&c}; delete p; delete p; return c; }
static_assert(o() == 1);
