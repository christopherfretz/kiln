// [basic.life]/7, [class.union.general]/2, [expr.const]/10.9: the address of
// a subobject reached through a union member (`u.s.k`, `u.arr[1]`,
// `(&u.s)->k`, a union inside a class inside a union, a base class of a
// member) designates that member at each union level, even where another
// member holds an object of the same type at the same offset, so reading
// through it, or through a pointer projected from it, while another member
// is active is rejected as clang 22 and GCC 16.1 do (spec/oven.md "Union
// member designators").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:86 note "read of member 's' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 43:15 note "in call to 'member_of_member()'"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:83 note "read of member 's' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 45:15 note "in call to 'second_member()'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:78 note "read of member 'arr' of union with active member 's' is not allowed in a constant expression"
//@ diag: 47:15 note "in call to 'element()'"
//@ diag: 49:15 error "static assertion expression is not an integral constant expression"
//@ diag: 48:79 note "read of member 's' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 49:15 note "in call to 'arrow()'"
//@ diag: 51:15 error "static assertion expression is not an integral constant expression"
//@ diag: 50:98 note "read of member 's' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 51:15 note "in call to 'nested_inner()'"
//@ diag: 53:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:90 note "read of member 'o' of union with active member 'z' is not allowed in a constant expression"
//@ diag: 53:15 note "in call to 'nested_outer()'"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 54:64 note "read of member 'd' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 55:15 note "in call to 'base()'"
//@ diag: 66:15 error "static assertion expression is not an integral constant expression"
//@ diag: 65:75 note "read of member 's' of union with active member 'arr' is not allowed in a constant expression"
//@ diag: 66:15 note "in call to 'projected()'"
struct S { int k; int j; };
union U { S s; int arr[2]; };
struct O { int pad; U u; };
union N { O o; int z[4]; };
struct B { int k; };
struct D : B { };
union UB { D d; int i; };
constexpr int member_of_member() { U u{.arr = {1, 2}}; const int *p = &u.s.k; return *p; }
static_assert(member_of_member() == 1);
constexpr int second_member() { U u{.arr = {1, 2}}; const int *p = &u.s.j; return *p; }
static_assert(second_member() == 2);
constexpr int element() { U u{.s = {1, 2}}; const int *p = &u.arr[1]; return *p; }
static_assert(element() == 2);
constexpr int arrow() { U u{.arr = {1, 2}}; const int *p = &(&u.s)->k; return *p; }
static_assert(arrow() == 1);
constexpr int nested_inner() { N n{.o = {0, {.arr = {1, 2}}}}; const int *p = &n.o.u.s.k; return *p; }
static_assert(nested_inner() == 1);
constexpr int nested_outer() { N n{.z = {0, 1, 2, 3}}; const int *p = &n.o.u.s.k; return *p; }
static_assert(nested_outer() == 1);
constexpr int base() { UB u{.i = 1}; const B *p = &u.d; return p->k; }
static_assert(base() == 1);
// Accepted: the chain's members are the active ones.
constexpr int ok_member() { U u{.s = {1, 2}}; const int *p = &u.s.k; const int *q = &(&u.s)->j; return *p + *q; }
static_assert(ok_member() == 3);
constexpr int ok_element() { U u{.arr = {1, 2}}; const int *p = &u.arr[1]; return *p; }
static_assert(ok_element() == 2);
constexpr int ok_nested() { N n{.o = {0, {.s = {1, 2}}}}; const int *p = &n.o.u.s.k; return *p; }
static_assert(ok_nested() == 1);
constexpr int ok_base() { UB u{.d = {}}; const B *p = &u.d; return p->k; }
static_assert(ok_base() == 0);
constexpr int projected() { U u{.arr = {1, 2}}; const S *p = &u.s; return p->k; }
static_assert(projected() == 1);
constexpr int ok_projected() { U u{.s = {1, 2}}; const S *p = &u.s; return p->j; }
static_assert(ok_projected() == 2);
