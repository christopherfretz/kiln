// [lex.string.uneval]: an unevaluated string literal (here, a static_assert
// message) cannot have an encoding prefix, including when it comes from a
// macro.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:21 error "an unevaluated string literal cannot have an encoding prefix"
//@ diag: 10:13 note "expanded from macro 'MSG'"
//@ diag: 14:21 error "an unevaluated string literal cannot have an encoding prefix"
//@ diag: 11:17 note "expanded from macro 'WIDE'"
#define MSG u8"message"
#define WIDE(s) L ## s
static_assert(true, "plain" " concatenated");
static_assert(true, MSG);
static_assert(true, WIDE("wide"));
