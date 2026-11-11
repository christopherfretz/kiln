// GNU array initialization from a compound literal is diagnosed under
// -pedantic (-Wgnu-compound-literal-initializer); ISO C allows only a
// brace list or a string literal (C23 6.7.10p16).
//@ expect: compile-ok
//@ args: -pedantic
//@ strict-diags
//@ diag: 9:29 warning "initialization of an array of type 'const unsigned short[]' from a compound literal of type 'const unsigned short[2]' is a GNU extension"
//@ diag: 10:19 warning "initialization of an array of type 'int[]' from a compound literal of type 'int[3]' is a GNU extension"
static const unsigned short array[] = (const unsigned short []) { 0x0D2B, 7 };
int f(void) { int b[] = (int[]){1,2,3}; return b[2] + (int)sizeof b; }
