// [expr.const]/10.8, [expr.add]/4: clang 22 rejects binding a reference
// variable to the object past the end of an array (via '*', '[]', a cast,
// 'auto &'), while forming the pointer and passing it by reference are
// accepted (WP-948 siblings of reference-past-end-clang22).
//@ std: c++23
//@ requires: clang>=22
//@ expect: compile-fail
//@ diag: 24:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:44 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 24:15 note "in call to 'f2()'"
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 26:40 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 27:15 note "in call to 'f3()'"
//@ diag: 29:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:45 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 29:15 note "in call to 'f4()'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:45 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 34:15 note "in call to 'f6()'"
int arr[3];
constexpr bool f1() { int *p = &arr[3]; return true; }
static_assert(f1());
constexpr bool f2() { int a[3]{}; int &r = *(a + 3); return true; }
static_assert(f2());
struct S { int x; };
constexpr bool f3() { S s[2]{}; S &r = s[2]; return true; }
static_assert(f3());
constexpr bool f4() { int a[3]{}; int &&r = static_cast<int&&>(a[3]); return true; }
static_assert(f4());
constexpr int g(int &r) { return 0; }
constexpr bool f5() { int a[3]{}; g(a[3]); return true; }
static_assert(f5());
constexpr bool f6() { int a[3]{}; auto &r = a[3]; return true; }
static_assert(f6());
