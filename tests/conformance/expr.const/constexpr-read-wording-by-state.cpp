// [basic.life]/1, [class.base.init]/13, [dcl.init.aggr]/7: the wording of
// a read that fails in a constant expression, by the state of the object
// read (spec/oven.md "Read diagnostics by object state"). A subobject
// whose initialization has not begun (a later member read by a
// mem-initializer, a base read while its constructor's arguments are
// evaluated, a later element read by a local's own initializer, also
// from a call it makes) is "outside its lifetime", as clang 22 says; a
// default-initialized local or member is "uninitialized". Found running
// GCC's C++ testsuite (WP-1494).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:46 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 73:15 note "in call to 'uninit_local()'"
//@ diag: 74:40 warning "field 'b' is uninitialized when used here [-Wuninitialized]"
//@ diag: 76:15 error "static assertion expression is not an integral constant expression"
//@ diag: 74:40 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 75:34 note "in call to 'M()'"
//@ diag: 76:15 note "in call to 'later_member()'"
//@ diag: 78:41 warning "field 'w' is uninitialized when used here [-Wuninitialized]"
//@ diag: 80:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:41 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 79:42 note "in call to 'D()'"
//@ diag: 80:15 note "in call to 'later_member_in_base()'"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 81:81 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 81:34 note "in call to 'this->read()'"
//@ diag: 82:33 note "in call to 'E()'"
//@ diag: 83:15 note "in call to 'base_before()'"
//@ diag: 85:15 error "static assertion expression is not an integral constant expression"
//@ diag: 84:42 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 85:15 note "in call to 'array_elem()'"
//@ diag: 88:15 error "static assertion expression is not an integral constant expression"
//@ diag: 87:53 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 88:15 note "in call to 'after_dtor()'"
//@ diag: 90:15 error "static assertion expression is not an integral constant expression"
//@ diag: 89:73 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 90:15 note "in call to 'after_destroy_at()'"
//@ diag: 92:15 error "static assertion expression is not an integral constant expression"
//@ diag: 91:70 note "read of heap allocated object that has been deleted"
//@ diag: 92:15 note "in call to 'after_delete()'"
//@ diag: 93:51 warning "reference to stack memory associated with local variable 'x' returned [-Wreturn-stack-address]"
//@ diag: 95:15 error "static assertion expression is not an integral constant expression"
//@ diag: 94:35 note "read of variable whose lifetime has ended"
//@ diag: 95:15 note "in call to 'dangling()'"
//@ diag: 93:37 note "declared here"
//@ diag: 97:15 error "static assertion expression is not an integral constant expression"
//@ diag: 96:78 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 97:15 note "in call to 'dangling_scope()'"
//@ diag: 101:66 warning "field 'b' is uninitialized when used here [-Wuninitialized]"
//@ diag: 103:15 error "static assertion expression is not an integral constant expression"
//@ diag: 101:60 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 102:36 note "in call to 'Self()'"
//@ diag: 103:15 note "in call to 'self_member()'"
//@ diag: 105:15 error "static assertion expression is not an integral constant expression"
//@ diag: 104:94 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 105:15 note "in call to 'uninit_member()'"
//@ diag: 107:15 error "static assertion expression is not an integral constant expression"
//@ diag: 106:32 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 107:15 note "in call to 'scal()'"
//@ diag: 109:27 warning "variable 'p' is uninitialized when used within its own initialization [-Wuninitialized]"
//@ diag: 110:15 error "static assertion expression is not an integral constant expression"
//@ diag: 109:27 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 110:15 note "in call to 'agg()'"
//@ diag: 115:15 error "static assertion expression is not an integral constant expression"
//@ diag: 113:41 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 114:40 note "in call to 'id(a[1])'"
//@ diag: 115:15 note "in call to 'arr_call()'"
using size_t = decltype(sizeof 0);
namespace std { template <class T> constexpr void destroy_at(T *p) { p->~T(); } }
constexpr int uninit_local() { int x; return x; }
static_assert(uninit_local() == 0);
struct M { int a, b; constexpr M() : a(b + 1), b(1) {} };
constexpr int later_member() { M m; return m.a; }
static_assert(later_member() == 0);
struct B { int v; constexpr B(int x) : v(x) {} };
struct D : B { int w; constexpr D() : B(w), w(2) {} };
constexpr int later_member_in_base() { D d; return d.v; }
static_assert(later_member_in_base() == 0);
struct E : B { constexpr E() : B(read()) {} constexpr int read() const { return v; } };
constexpr int base_before() { E e; return e.v; }
static_assert(base_before() == 0);
constexpr int array_elem() { int a[3] = {a[1], 2, 3}; return a[0]; }
static_assert(array_elem() == 0);
struct T { int v; constexpr ~T() {} };
constexpr int after_dtor() { T t{1}; t.~T(); return t.v; }
static_assert(after_dtor() == 0);
constexpr int after_destroy_at() { T t{1}; std::destroy_at(&t); int r = t.v; return r; }
static_assert(after_destroy_at() == 0);
constexpr int after_delete() { int *p = new int(1); delete p; return *p; }
static_assert(after_delete() == 0);
constexpr const int &dangle() { int x = 1; return x; }
constexpr int dangling() { return dangle(); }
static_assert(dangling() == 0);
constexpr int dangling_scope() { const int *p; { int x = 1; p = &x; } return *p; }
static_assert(dangling_scope() == 0);
struct Mv { int *p; constexpr Mv(int *q) : p(q) {} constexpr Mv(Mv &&o) : p(o.p) { o.p = nullptr; } };
constexpr int moved_from() { int x = 1; Mv a(&x); Mv b(static_cast<Mv &&>(a)); return a.p == nullptr; }
static_assert(moved_from() == 1);
struct Self { int a; int b; constexpr Self() : a(1), b(a + this->b) {} };
constexpr int self_member() { Self s; return s.b; }
static_assert(self_member() == 0);
constexpr int uninit_member() { struct Q { int a; int b; constexpr Q() : a(1) {} } q; return q.b; }
static_assert(uninit_member() == 0);
constexpr int scal() { int x = x + 1; return x; }
static_assert(scal() == 0);
struct P { int a, b; };
constexpr int agg() { P p{p.b, 1}; return p.a; }
static_assert(agg() == 0);
constexpr int agg_ok() { P p{1, p.a}; return p.b; }
static_assert(agg_ok() == 1);
constexpr int id(const int &r) { return r; }
constexpr int arr_call() { int a[2] = {id(a[1]), 2}; return a[0]; }
static_assert(arr_call() == 0);
