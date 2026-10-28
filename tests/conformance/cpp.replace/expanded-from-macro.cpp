// [cpp.replace], [cpp.rescan]: a diagnostic on a token produced by macro
// expansion is located at the token's spelling, with an "expanded from
// macro" note for each expansion it went through.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:17 error "typedef redefinition with different types ('long' vs 'int')"
//@ diag: 14:13 note "previous definition is here"
//@ diag: 16:1 error "typedef redefinition with different types ('short' vs 'int')"
//@ diag: 13:28 note "expanded from macro 'WRAP'"
//@ diag: 14:13 note "previous definition is here"
#define LONG_T long
#define DECLARE(T, name) typedef T name;
#define WRAP(x) DECLARE(x, Alias)
typedef int Alias;
DECLARE(LONG_T, Alias)
WRAP(short)
