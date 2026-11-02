// GNU __null (spec/sema.md §5.8) has type long: converting it to int and
// to void* are both conversions, so a call choosing between them is
// ambiguous ([over.ics.rank]).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:9 error "call to 'g' is ambiguous"
//@ diag: 9:5 note "candidate function"
//@ diag: 10:5 note "candidate function"
int g(int);
int g(void *);
int r = g(__null);
