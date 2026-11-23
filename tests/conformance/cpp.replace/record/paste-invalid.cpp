// [cpp.concat]/3: a paste that forms no valid preprocessing token is an
// error. Its made row is flagged invalid and lists the tokens joined
// before the rejected one, then the rejected one, which is delivered on
// its own (spec/pp.md "Preprocessing record", made tokens).
//@ expect: compile-fail
//@ strict-diags
//@ pp-record: paste-invalid.record
//@ diag: 15:9 error "pasting formed '1+', an invalid preprocessing token"
//@ diag: 12:18 note "expanded from macro 'BAD'"
//@ diag: 16:9 error "pasting formed 'xy-', an invalid preprocessing token"
//@ diag: 13:27 note "expanded from macro 'BAD2'"
#define BAD(a) a ## +
#define BAD2(a, b) a ## b ## -
int xy;
int i = BAD(1) 2;
int j = BAD2(x, y) 3;
