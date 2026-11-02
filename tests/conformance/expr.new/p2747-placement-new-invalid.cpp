// [expr.new], [expr.const]/5.18 (C++26, P2747R2): a placement new is not a
// constant expression when its pointer does not designate storage that can
// hold the new object: an object of another type or a smaller array (clang
// 22: "placement new would change type of storage"), a const object, a null
// or past-the-end pointer, a deleted heap object, an object visible outside
// the evaluation; and the array bound is checked as for an allocating new.
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:42 note "placement new would change type of storage from 'int' to 'int[3]'"
//@ diag: 61:15 note "in call to 'decayed()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:51 note "placement new would change type of storage from 'long' to 'int'"
//@ diag: 63:15 note "in call to 'other_type()'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:50 note "placement new would change type of storage from 'S' to 'int'"
//@ diag: 65:15 note "in call to 'via_void()'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:32 note "placement new would change type of storage from 'B' to 'D'"
//@ diag: 67:15 note "in call to 'derived()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:48 note "placement new would change type of storage from 'int[2]' to 'int[3]'"
//@ diag: 69:15 note "in call to 'too_many(3)'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:61 note "cannot allocate array; evaluated array bound -1 is negative"
//@ diag: 71:15 note "in call to 'negative(-1)'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:54 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 73:15 note "in call to 'to_const()'"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 74:33 note "construction of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 75:15 note "in call to 'null()'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 76:39 note "construction of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 77:15 note "in call to 'past_end()'"
//@ diag: 79:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:58 note "construction of heap allocated object that has been deleted"
//@ diag: 79:15 note "in call to 'deleted()'"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:58 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 81:15 note "in call to 'uninit()'"
//@ diag: 84:15 error "static assertion expression is not an integral constant expression"
//@ diag: 83:26 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 84:15 note "in call to 'global()'"
//@ diag: 86:15 error "static assertion expression is not an integral constant expression"
//@ diag: 85:39 note "this placement new expression is not supported in constant expressions "
//@ diag: 86:15 note "in call to 'other_form()'"
//@ diag: 89:15 error "static assertion expression is not an integral constant expression"
//@ diag: 87:61 note "placement new would change type of storage from 'int' to 'float'"
//@ diag: 89:15 note "in call to 'retype<int, float>()'"
namespace std { using size_t = decltype(sizeof 0); }
void *operator new(std::size_t, void *p) noexcept;
void *operator new[](std::size_t, void *p) noexcept;
void *operator new(std::size_t, int) noexcept;
struct S { int a, b; constexpr S(int x) : a(x), b(x + 1) {} };
struct B { int v = 1; };
struct D : B { int w = 2; };

constexpr int decayed() { int a[3] = {}; new (a) int[3]{1, 2, 3}; return a[2]; }
static_assert(decayed() == 3);
constexpr int other_type() { long x = 0; int *p = new (&x) int(1); return *p; }
static_assert(other_type() == 1);
constexpr int via_void() { S s(1); void *v = &s; new (v) int(2); return s.a; }
static_assert(via_void() == 2);
constexpr int derived() { B b; new (&b) D; return 0; }
static_assert(derived() == 0);
constexpr int too_many(int n) { int a[2] = {}; new (&a) int[n]; return 0; }
static_assert(too_many(3) == 0);
constexpr int negative(int n) { int a[2] = {}; new (&a) int[n]; return 0; }
static_assert(negative(-1) == 0);
constexpr int to_const() { const int x = 0; int *p = new ((void *)&x) int(1); return *p; }
static_assert(to_const() == 1);
constexpr int null() { int *p = new (nullptr) int(7); return *p; }
static_assert(null() == 7);
constexpr int past_end() { int x = 0; new (&x + 1) int(4); return 0; }
static_assert(past_end() == 0);
constexpr int deleted() { int *p = new int(1); delete p; new (p) int(2); return 0; }
static_assert(deleted() == 0);
constexpr int uninit() { int x = 1; new (&x) int; return x; }
static_assert(uninit() == 0);
int g;
constexpr int global() { new (&g) int(1); return 0; }
static_assert(global() == 0);
constexpr int other_form() { int *p = new (1) int(2); delete p; return 0; }
static_assert(other_form() == 0);
template <class T, class U> constexpr int retype() { T t{}; new (&t) U(); return 0; }
static_assert(retype<int, int>() == 0);
static_assert(retype<int, float>() == 0);
