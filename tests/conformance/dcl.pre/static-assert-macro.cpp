// [dcl.pre]/12: a failed static_assert produced by a macro is diagnosed at
// the condition, with "expanded from macro" notes.
//@ expect: compile-fail
//@ diag: 13:7 error "static assertion failed due to requirement '1 + 1 == 3'"
//@ diag: 13:13 note "expression evaluates to '2 == 3'"
//@ diag: 15:1 error "static assertion failed due to requirement 'sizeof(int) == 2'"
//@ diag: 11:32 note "expanded from macro 'CHECK_SIZE'"
//@ diag: 15:1 note "expression evaluates to '4 == 2'"
//@ diag: 11:42 note "expanded from macro 'CHECK_SIZE'"
#define CHECK(e) static_assert(e, #e)
#define CHECK_SIZE(T, n) CHECK(sizeof(T) == n)
CHECK(1 + 1 == 2);
CHECK(1 + 1 == 3);
CHECK_SIZE(char, 1);
CHECK_SIZE(int, 2);
