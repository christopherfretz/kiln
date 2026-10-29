// [dcl.constexpr]/6, [dcl.init.general]/7, [class.default.ctor]: a
// constexpr variable of class type may be default-initialized when the
// default constructor (implicit or constexpr) initializes every member;
// the result must be a constant expression, and the class must be
// const-default-constructible.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:13 error "default initialization of an object of const type 'const P' without a user-provided default constructor"
//@ diag: 25:13 error "constexpr variable 'q' must be initialized by a constant expression"
//@ diag: 25:13 note "subobject 'x' is not initialized"
//@ diag: 24:16 note "subobject declared here"
//@ diag: 27:13 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 27:13 note "pointer to heap-allocated object is not a constant expression"
//@ diag: 26:38 note "heap allocation performed here"
struct D { constexpr ~D() {} };
constexpr D d0;
struct E {};
constexpr E e0;
struct F { int x = 1; constexpr ~F() {} };
constexpr F f0;
static_assert(f0.x == 1);
struct P { int x; };
constexpr P p;
struct Q { int x; constexpr Q() {} };
constexpr Q q;
struct H { int *p; constexpr H() : p(new int(1)) {} };
constexpr H h;
struct R { int x = 2; int y; constexpr R() : y(x + 1) {} };
constexpr R r;
static_assert(r.y == 3);
