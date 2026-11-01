// [expr.const]/10.9, [class.copy.ctor]/14, [class.copy.assign]/12 (WP-224):
// a trivial copy/move constructor or assignment operator of a class reads
// its source whole (clang's `handleTrivialCopy`), so every subobject of the
// value must be initialized: "subobject 'b' is not initialized" at the
// source argument, then the trivial call's own frame note ('S(s)',
// 't.operator=(s)', 'h.s.operator=(s)' through an anonymous union) and
// "subobject declared here". A base class is walked into, an array member
// is named by the member, unnamed bit-fields, padding and empty classes
// are not read, a union is copied as its object representation (no check),
// and a union member is checked only if it is active.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 96:15 error "static assertion expression is not an integral constant expression"
//@ diag: 95:49 note "subobject 'b' is not initialized"
//@ diag: 95:49 note "in call to 'S(s)'"
//@ diag: 96:15 note "in call to 'ctor_copy()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 98:15 error "static assertion expression is not an integral constant expression"
//@ diag: 97:47 note "subobject 'a' is not initialized"
//@ diag: 97:47 note "in call to 'S(s)'"
//@ diag: 98:15 note "in call to 'ctor_direct()'"
//@ diag: 84:16 note "subobject declared here"
//@ diag: 100:15 error "static assertion expression is not an integral constant expression"
//@ diag: 99:42 note "subobject 'a' is not initialized"
//@ diag: 99:38 note "in call to 't.operator=(s)'"
//@ diag: 100:15 note "in call to 'assign()'"
//@ diag: 84:16 note "subobject declared here"
//@ diag: 102:15 error "static assertion expression is not an integral constant expression"
//@ diag: 101:68 note "subobject 'b' is not initialized"
//@ diag: 101:56 note "in call to 't.operator=(s)'"
//@ diag: 102:15 note "in call to 'assign_explicit()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 104:15 error "static assertion expression is not an integral constant expression"
//@ diag: 103:71 note "subobject 'b' is not initialized"
//@ diag: 103:66 note "in call to '*p.operator=(s)'"
//@ diag: 104:15 note "in call to 'assign_pointer()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 106:15 error "static assertion expression is not an integral constant expression"
//@ diag: 105:77 note "subobject 'b' is not initialized"
//@ diag: 105:64 note "in call to 'p->operator=(s)'"
//@ diag: 106:15 note "in call to 'assign_arrow()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 108:15 error "static assertion expression is not an integral constant expression"
//@ diag: 107:49 note "subobject 'b' is not initialized"
//@ diag: 107:49 note "in call to 'S(s)'"
//@ diag: 108:15 note "in call to 'move_ctor()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 110:15 error "static assertion expression is not an integral constant expression"
//@ diag: 109:51 note "subobject 'a' is not initialized"
//@ diag: 109:51 note "in call to 'P(p)'"
//@ diag: 110:15 note "in call to 'array_member()'"
//@ diag: 87:16 note "subobject declared here"
//@ diag: 112:15 error "static assertion expression is not an integral constant expression"
//@ diag: 111:51 note "subobject 'x' is not initialized"
//@ diag: 111:51 note "in call to 'D(d)'"
//@ diag: 112:15 note "in call to 'base_uninit()'"
//@ diag: 85:16 note "subobject declared here"
//@ diag: 114:15 error "static assertion expression is not an integral constant expression"
//@ diag: 113:60 note "subobject 'b' is not initialized"
//@ diag: 113:60 note "in call to 'Bits(b)'"
//@ diag: 114:15 note "in call to 'named_bitfield()'"
//@ diag: 88:39 note "subobject declared here"
//@ diag: 128:15 error "static assertion expression is not an integral constant expression"
//@ diag: 127:75 note "subobject 'b' is not initialized"
//@ diag: 127:75 note "in call to 'HU2(h)'"
//@ diag: 128:15 note "in call to 'union_member_partial()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 132:15 error "static assertion expression is not an integral constant expression"
//@ diag: 131:75 note "subobject 'b' is not initialized"
//@ diag: 131:69 note "in call to 'h.s.operator=(s)'"
//@ diag: 132:15 note "in call to 'union_member_assign_uninit()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 135:15 error "static assertion expression is not an integral constant expression"
//@ diag: 133:43 note "subobject 'b' is not initialized"
//@ diag: 133:43 note "in call to 'S(s)'"
//@ diag: 134:34 note "in call to 'make()'"
//@ diag: 135:15 note "in call to 'returned()'"
//@ diag: 84:23 note "subobject declared here"
//@ diag: 138:15 error "static assertion expression is not an integral constant expression"
//@ diag: 137:58 note "subobject 'b' is not initialized"
//@ diag: 137:58 note "in call to 'S(s)'"
//@ diag: 138:15 note "in call to 'argument()'"
//@ diag: 84:23 note "subobject declared here"
struct S { int a; int b; };
struct B { int x; };
struct D : B { int y; };
struct P { int a[2]; };
struct Bits { int a : 3; int : 5; int b : 4; };
struct Pad { char c; int i; };
struct E {};
union U { int i; float f; };
struct HU { int k; U u; };
struct HU2 { int k; union { S s; int n; }; };

constexpr int ctor_copy() { S s; s.a = 1; S t = s; return t.a; }
static_assert(ctor_copy() == 1);
constexpr int ctor_direct() { S s; s.b = 1; S t(s); return t.b; }
static_assert(ctor_direct() == 1);
constexpr int assign() { S s; S t{}; t = s; return t.a; }
static_assert(assign() == 0);
constexpr int assign_explicit() { S s; s.a = 1; S t{}; t.operator=(s); return t.a; }
static_assert(assign_explicit() == 1);
constexpr int assign_pointer() { S s; s.a = 1; S t{}; S* p = &t; *p = s; return t.a; }
static_assert(assign_pointer() == 1);
constexpr int assign_arrow() { S s; s.a = 1; S t{}; S* p = &t; p->operator=(s); return t.a; }
static_assert(assign_arrow() == 1);
constexpr int move_ctor() { S s; s.a = 1; S t = static_cast<S&&>(s); return t.a; }
static_assert(move_ctor() == 1);
constexpr int array_member() { P p; p.a[0] = 1; P q(p); return q.a[0]; }
static_assert(array_member() == 1);
constexpr int base_uninit() { D d; d.y = 1; D e = d; return e.y; }
static_assert(base_uninit() == 1);
constexpr int named_bitfield() { Bits b; b.a = 1; Bits c = b; return c.a; }
static_assert(named_bitfield() == 1);
constexpr int padded() { Pad p; p.c = 1; p.i = 2; Pad q = p; q = p; return q.i; }
static_assert(padded() == 2);
constexpr int bitfields_ok() { Bits b; b.a = 1; b.b = 2; Bits c = b; c = b; return c.b; }
static_assert(bitfields_ok() == 2);
constexpr int empty() { E e; E f = e; f = e; return 0; }
static_assert(empty() == 0);
constexpr int union_copy() { U u; U v = u; v = u; return 0; }
static_assert(union_copy() == 0);
constexpr int union_member_unset() { HU h; h.k = 1; HU g = h; return g.k; }
static_assert(union_member_unset() == 1);
constexpr int union_member_set() { HU h; h.k = 1; h.u.i = 2; HU g = h; return g.u.i; }
static_assert(union_member_set() == 2);
constexpr int union_member_partial() { HU2 h; h.k = 1; h.s.a = 2; HU2 g = h; return g.k; }
static_assert(union_member_partial() == 1);
constexpr int union_member_assign() { HU2 h{}; S s{1, 2}; h.s = s; return h.s.b; }
static_assert(union_member_assign() == 2);
constexpr int union_member_assign_uninit() { HU2 h{}; S s; s.a = 1; h.s = s; return h.s.a; }
static_assert(union_member_assign_uninit() == 1);
constexpr S make() { S s; s.a = 1; return s; }
constexpr int returned() { S t = make(); return t.a; }
static_assert(returned() == 1);
constexpr int by_value(S s) { return s.a; }
constexpr int argument() { S s; s.a = 1; return by_value(s); }
static_assert(argument() == 1);
