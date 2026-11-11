// C89 6.3.3.2 / C99 6.5.3.2p3: `&*p` of a `void *`. clang warns about
// the indirection (-Wvoid-ptr-dereference) except right after `&` from
// C99 (`IsAfterAmp`; a parenthesized `&(*p)` still warns) and in an
// unevaluated operand; before C99 taking the address of the `void`
// lvalue is a pedantic extension, as is `&x` of a `void` object in any
// mode (WP-1312, WP-1306 follow-up).
//@ std: c89
//@ args: -pedantic
//@ strict-diags
//@ diag: 1:1 warning "// comments are not allowed in this language"
//@ diag: 19:11 warning "ISO C forbids taking the address of an expression of type 'void'"
//@ diag: 21:14 warning "ISO C does not allow indirection on operand of type 'void *'"
//@ diag: 21:13 warning "ISO C forbids taking the address of an expression of type 'void'"
//@ diag: 22:15 warning "ISO C does not allow indirection on operand of type 'void *'"
//@ diag: 22:13 warning "ISO C forbids taking the address of an expression of type 'void'"
//@ diag: 23:15 warning "invalid application of 'sizeof' to a void type"
//@ diag: 24:9 warning "ISO C does not allow indirection on operand of type 'void *'"
extern void x;
void *y = &x;
void f(void *p) {
  void *q = &*p;
  void *r = &(*p);
  (void)sizeof(*p);
  (void)*p;
  (void)q; (void)r;
}
