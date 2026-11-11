// [lex.phases]/1.1: -Wno-invalid-source-encoding silences the warning for
// source bytes that are not valid UTF-8 in an ordinary literal (clang).
//@ expect: compile-ok
//@ args: -Wno-invalid-source-encoding
//@ strict-diags
const char *a = "café";
char b = 'é';
static_assert(sizeof("café") == 5);
