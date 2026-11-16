// [expr.const]/5.16, [dcl.type.cv]/4: a const temporary (bound to a
// reference), an object created by `new const T` and an element of a
// const array of class type are const once initialized; writing them
// through a `const_cast` is not a constant expression. Found running
// GCC's C++ testsuite (WP-1468).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:15 error "constexpr function never produces a constant expression"
//@ diag: 27:63 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:63 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 28:15 note "in call to 'temp()'"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:72 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 30:15 note "in call to 'heap()'"
//@ diag: 31:15 error "constexpr function never produces a constant expression"
//@ diag: 31:80 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:80 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 32:15 note "in call to 'heap2()'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:61 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 34:15 note "in call to 'arr()'"
struct A { int n = 1; };
struct H { const int h = 1; int k = 0; };
constexpr int temp() { const A &r = A{}; const_cast<A &>(r).n = 2; return 0; }
static_assert(temp() == 0);
constexpr int heap() { const A *p = new const A; const_cast<A *>(p)->n = 2; delete p; return 0; }
static_assert(heap() == 0);
constexpr int heap2() { const int *p = new const int(1); *const_cast<int *>(p) = 2; delete p; return 0; }
static_assert(heap2() == 0);
constexpr int arr() { const A a[2]; const_cast<A &>(a[1]).n = 2; return 0; }
static_assert(arr() == 0);
