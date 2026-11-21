// An encoding prefix on a string literal that is not evaluated (an
// attribute's string argument, a `_Static_assert` message) is valid C;
// clang warns that it has no effect (-Winvalid-unevaluated-string), once
// per prefixed piece at its prefix, and uses the string as an ordinary
// one. spec/lex.md, WP-1805.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:27 warning "encoding prefix 'u8' on an unevaluated string literal has no effect"
//@ diag: 16:24 warning "encoding prefix 'u8' on an unevaluated string literal has no effect"
//@ diag: 17:25 warning "encoding prefix 'u8' on an unevaluated string literal has no effect"
//@ diag: 18:27 warning "encoding prefix 'u8' on an unevaluated string literal has no effect"
//@ diag: 19:27 warning "encoding prefix 'L' on an unevaluated string literal has no effect"
//@ diag: 20:31 warning "encoding prefix 'u' on an unevaluated string literal has no effect"
//@ diag: 21:19 warning "encoding prefix 'u8' on an unevaluated string literal has no effect"
__attribute__((deprecated(u8"x"))) void f1(void);
__attribute__((section(u8".text.x"))) void f2(void) {}
__attribute__((annotate(u8"x"))) void f3(void);
__attribute__((visibility(u8"hidden"))) void f4(void);
__attribute__((deprecated(L"x"))) void f5(void);
__attribute__((deprecated("a" u"b"))) void f6(void);
_Static_assert(1, u8"x");
