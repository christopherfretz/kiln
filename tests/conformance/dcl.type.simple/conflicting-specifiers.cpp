// [dcl.type.simple]: at most one of the size/sign/type keywords of each kind
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:6 error "cannot combine with previous 'long' declaration specifier"
//@ diag: 8:10 error "cannot combine with previous 'unsigned' declaration specifier"
//@ diag: 9:5 error "cannot combine with previous 'int' declaration specifier"
long short a;
unsigned signed b;
int char c;
