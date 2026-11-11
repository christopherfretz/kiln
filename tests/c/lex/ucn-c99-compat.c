// C11 6.4.2.1 vs C99 Annex D: -Wc99-compat flags each identifier
// character C11 accepts but C99 does not (anywhere, or initially).
//@ std: c11
//@ args: -Wc99-compat
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:5 warning "using this character in an identifier is incompatible with C99"
//@ diag: 11:5 warning "starting an identifier with this character is incompatible with C99"
//@ diag: 12:6 warning "using this character in an identifier is incompatible with C99"
int ¨a;
int \u0660b;
int c\U0001F600, d٠, \u00C0;
