// [basic.life]/7, [class.copy.ctor]/14, [class.copy.assign]/12 (WP-241):
// reading a destroyed or uninitialized object through a trivial copy/move
// constructor or assignment is not a constant expression; clang reports the
// read at the call and adds the call's frame note ("in call to 'T(t)'",
// "in call to '&u->operator=(t)'"), with the union variant.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 57:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:47 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 52:47 note "in call to 'T(t)'"
//@ diag: 57:15 note "in call to 'destroyed()'"
//@ diag: 58:15 error "static assertion expression is not an integral constant expression"
//@ diag: 53:33 note "subobject 'v' is not initialized"
//@ diag: 53:33 note "in call to 'T(t)'"
//@ diag: 58:15 note "in call to 'uninit()'"
//@ diag: 50:16 note "subobject declared here"
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:47 note "subobject 'b' is not initialized"
//@ diag: 54:47 note "in call to 'U(t)'"
//@ diag: 59:15 note "in call to 'partial()'"
//@ diag: 51:24 note "subobject declared here"
//@ diag: 60:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:62 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 55:58 note "in call to 'u.operator=(t)'"
//@ diag: 60:15 note "in call to 'assign_destroyed()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 56:33 note "subobject 'v' is not initialized"
//@ diag: 56:33 note "in call to 'T(t[1])'"
//@ diag: 61:15 note "in call to 'arr()'"
//@ diag: 50:16 note "subobject declared here"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:63 note "member call on object outside its lifetime is not allowed in a constant expression"
//@ diag: 63:15 note "in call to 'assign_to_destroyed()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:53 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 65:53 note "in call to 'V(a)'"
//@ diag: 66:15 note "in call to 'union_destroyed()'"
//@ diag: 68:15 error "static assertion expression is not an integral constant expression"
//@ diag: 67:58 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 67:54 note "in call to 'b.operator=(a)'"
//@ diag: 68:15 note "in call to 'union_assign()'"
//@ diag: 70:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:67 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 69:55 note "in call to 'u.operator=(t)'"
//@ diag: 70:15 note "in call to 'member_assign()'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:52 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 72:52 note "in call to 'T(w.t)'"
//@ diag: 73:15 note "in call to 'nested()'"
struct T { int v; };
struct U { int a; char b; };
constexpr int destroyed() { T t{1}; t.~T(); T u(t); return u.v; }
constexpr int uninit() { T t; T u(t); return u.v; }
constexpr int partial() { U t; t.a = 1; U u = t; return u.a; }
constexpr int assign_destroyed() { T t{1}, u{2}; t.~T(); u = t; return u.v; }
constexpr int arr() { T t[2]; T u(t[1]); return u.v; }
static_assert(destroyed() == 1);
static_assert(uninit() == 1);
static_assert(partial() == 1);
static_assert(assign_destroyed() == 1);
static_assert(arr() == 1);
constexpr int assign_to_destroyed() { T t{1}, u{2}; u.~T(); u = t; return u.v; }
static_assert(assign_to_destroyed() == 1);
union V { int i; float f; };
constexpr int union_destroyed() { V a{1}; a.~V(); V b(a); return b.i; }
static_assert(union_destroyed() == 1);
constexpr int union_assign() { V a{1}, b{2}; a.~V(); b = a; return b.i; }
static_assert(union_assign() == 1);
constexpr int member_assign() { T t{1}, u{2}; t.~T(); u.operator=(t); return u.v; }
static_assert(member_assign() == 1);
struct W { T t; };
constexpr int nested() { W w{{1}}; w.t.~T(); T u = w.t; return u.v; }
static_assert(nested() == 1);
