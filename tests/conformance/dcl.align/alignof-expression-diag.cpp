// [expr.alignof]/1: the operand of alignof is a type-id; `alignof e` is a
// GNU extension (clang -Wgnu-alignof-expression, on by default, at the
// keyword; not for `__alignof`). Its value for an expression naming a
// variable or data member is the declaration's alignment, which for a
// data member needs the complete class: [class.mem.general]/8, a
// static_assert in the member-specification is no complete-class context.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:30 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 16:15 warning "'_Alignof' applied to an expression is a GNU extension"
//@ diag: 18:33 warning "'alignof' applied to an expression is a GNU extension"
//@ diag: 17:42 error "invalid application of 'alignof' to a field of a class still being defined"
//@ diag: 18:40 error "invalid application of 'alignof' to a field of a class still being defined"
alignas(16) char g;
static_assert(alignof(int) + alignof(g) == 20);
static_assert(_Alignof g == 16 && __alignof(g) == 16 && __alignof__ g == 16);
struct U { int i; static_assert(__alignof(i) == 4); };
struct V { int i; static_assert(alignof(i) == 4); };
struct W { char c; int f() { return __alignof(c); } };
