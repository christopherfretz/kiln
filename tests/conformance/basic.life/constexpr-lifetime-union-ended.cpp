// [basic.life]/1.4, /7, [class.union.general]/6, [expr.const]/5.18: after
// a union object's own destruction, assigning one of its members does not
// begin a lifetime (only a member access of a union object within its
// lifetime makes the member active), so the assignment is not constant.
// spec/oven.md "Object lifetime outside std::allocator storage".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:49 note "assignment to object outside its lifetime is not allowed in a constant expression"
//@ diag: 16:15 note "in call to 'whole_union()'"
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:57 note "assignment to object outside its lifetime is not allowed in a constant expression"
//@ diag: 21:15 note "in call to 'member_of_ended()'"
union V { int a; float b; };
constexpr int whole_union() { V v{1}; v.~V(); v.b = 2; return 1; }
static_assert(whole_union() == 1);

// A union member of a destroyed class object is outside its lifetime too.
struct S { V v; };
constexpr int member_of_ended() { S s{{1}}; s.~S(); s.v.b = 2; return 1; }
static_assert(member_of_ended() == 1);
