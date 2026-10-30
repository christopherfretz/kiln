// [basic.life]/7, [expr.const]/10.8, /10.18, [expr.delete]: in constant
// evaluation, a class object may not be used after its lifetime ended
// (scope exit, delete); deleting through the wrong form, twice, or a
// non-heap pointer is not a constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:67 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 32:15 note "in call to 'dangling()'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:66 note "read of heap allocated object that has been deleted"
//@ diag: 34:15 note "in call to 'after_delete()'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:52 note "delete of pointer that has already been deleted"
//@ diag: 36:15 note "in call to 'twice()'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 38:45 note "non-array delete used to delete pointer to array object of type 'T[2]'"
//@ diag: 39:15 note "in call to 'array_form()'"
//@ diag: 37:30 note "heap allocation performed here"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 41:45 note "array delete used to delete pointer to non-array object of type 'T'"
//@ diag: 42:15 note "in call to 'scalar_form()'"
//@ diag: 40:29 note "heap allocation performed here"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:33 note "delete of pointer '&t' that does not point to a heap-allocated object"
//@ diag: 44:15 note "in call to 'local()'"
//@ diag: 43:27 note "declared here"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:31 note "allocation performed here was not deallocated"
struct T { int v; constexpr T(int x) : v(x) {} constexpr ~T() {} };
constexpr int dangling() { const T *p; { T t(1); p = &t; } return p->v; }
static_assert(dangling() == 1);
constexpr int after_delete() { T *p = new T(2); delete p; return p->v; }
static_assert(after_delete() == 2);
constexpr int twice() { T *p = new T(3); delete p; delete p; return 0; }
static_assert(twice() == 0);
constexpr T *arr2() { return new T[2]{4, 5}; }
constexpr int array_form() { T *p = arr2(); delete p; return 0; }
static_assert(array_form() == 0);
constexpr T *one() { return new T(6); }
constexpr int scalar_form() { T *p = one(); delete[] p; return 0; }
static_assert(scalar_form() == 0);
constexpr int local() { T t(7); delete &t; return 0; }
static_assert(local() == 0);
constexpr int leak() { T *p = new T(8); return p->v; }
static_assert(leak() == 8);
constexpr int ok() { T *p = new T[3]{1, 2, 3}; int s = p[0].v + p[2].v; delete[] p; return s; }
static_assert(ok() == 4);
