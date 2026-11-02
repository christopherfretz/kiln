// [intro.compliance.general]/2.2 (a diagnostic for an ill-formed program),
// spec/diag.md §7.3: with the driver's default -ferror-limit=19, the 20th
// error (here `undeclared_after_limit`'s) is replaced by the locationless "fatal
// error: too many errors emitted, stopping now" (counted: "20 errors
// generated."), and everything after it is dropped. The 19th error keeps
// its notes. clang 22 and 19 agree.
//@ std: c++26
//@ strict-diags
//@ diag: 31:10 error "use of undeclared identifier 'undeclared0'"
//@ diag: 32:10 error "use of undeclared identifier 'undeclared1'"
//@ diag: 33:10 error "use of undeclared identifier 'undeclared2'"
//@ diag: 34:10 error "use of undeclared identifier 'undeclared3'"
//@ diag: 35:10 error "use of undeclared identifier 'undeclared4'"
//@ diag: 36:10 error "use of undeclared identifier 'undeclared5'"
//@ diag: 37:10 error "use of undeclared identifier 'undeclared6'"
//@ diag: 38:10 error "use of undeclared identifier 'undeclared7'"
//@ diag: 39:10 error "use of undeclared identifier 'undeclared8'"
//@ diag: 40:10 error "use of undeclared identifier 'undeclared9'"
//@ diag: 41:11 error "use of undeclared identifier 'undeclared10'"
//@ diag: 42:11 error "use of undeclared identifier 'undeclared11'"
//@ diag: 43:11 error "use of undeclared identifier 'undeclared12'"
//@ diag: 44:11 error "use of undeclared identifier 'undeclared13'"
//@ diag: 45:11 error "use of undeclared identifier 'undeclared14'"
//@ diag: 46:11 error "use of undeclared identifier 'undeclared15'"
//@ diag: 47:11 error "use of undeclared identifier 'undeclared16'"
//@ diag: 48:11 error "use of undeclared identifier 'undeclared17'"
//@ diag: 49:10 error "call to 'f' is ambiguous"
//@ diag: 30:6 note "candidate function"
//@ diag: 30:19 note "candidate function"
void f(int); void f(long);
int x0 = undeclared0;
int x1 = undeclared1;
int x2 = undeclared2;
int x3 = undeclared3;
int x4 = undeclared4;
int x5 = undeclared5;
int x6 = undeclared6;
int x7 = undeclared7;
int x8 = undeclared8;
int x9 = undeclared9;
int x10 = undeclared10;
int x11 = undeclared11;
int x12 = undeclared12;
int x13 = undeclared13;
int x14 = undeclared14;
int x15 = undeclared15;
int x16 = undeclared16;
int x17 = undeclared17;
int a = (f(1.0), 0);
int y = undeclared_after_limit;
int z = undeclared_dropped;
