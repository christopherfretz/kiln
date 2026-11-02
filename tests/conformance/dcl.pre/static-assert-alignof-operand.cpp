// [dcl.pre]/12: a failed static_assert prints its condition. A sizeof /
// alignof expression operand is printed after a space with its own
// parentheses (clang's StmtPrinter), never with an extra pair: `alignof
// (g)`, `__alignof (tv<char>)`, `sizeof ((g))`; a type operand prints as
// `alignof(int)`. [expr.alignof]/1, [expr.sizeof]/1.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-gnu-alignof-expression
//@ diag: 21:15 error "static assertion failed due to requirement 'alignof (g) == 1'"
//@ diag: 21:26 note "expression evaluates to '16 == 1'"
//@ diag: 22:15 error "static assertion failed due to requirement '__alignof (tv<char>) == 1'"
//@ diag: 22:35 note "expression evaluates to '16 == 1'"
//@ diag: 23:15 error "static assertion failed due to requirement 'alignof ((g)) + sizeof g == 1'"
//@ diag: 23:39 note "expression evaluates to '17 == 1'"
//@ diag: 24:15 error "static assertion failed due to requirement 'sizeof ((g)) + __alignof g + alignof(int) == 3'"
//@ diag: 24:58 note "expression evaluates to '21 == 3'"
alignas(16) char g;
template <class T> alignas(16) T tv;
// Expression operands (GNU extension for alignof; on by default, silenced
// above) and a type operand.
static_assert(alignof(g) == 1);
static_assert(__alignof(tv<char>) == 1);
static_assert(alignof((g)) + sizeof g == 1);
static_assert(sizeof((g)) + __alignof__ g + alignof(int) == 3);
