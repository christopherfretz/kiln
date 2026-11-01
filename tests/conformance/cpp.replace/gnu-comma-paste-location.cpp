// [cpp.replace.general], GNU extension: ', ## __VA_ARGS__' warns at the
// macro definition's ',' (variadic argument omitted) or '##' (present),
// without macro-expansion notes, once per expansion; an unused macro does
// not warn (clang MaybeRemoveCommaBeforeVaArgs; WP-230)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wgnu-zero-variadic-macro-arguments
//@ diag: 11:26 warning "token pasting of ',' and __VA_ARGS__ is a GNU extension"
//@ diag: 11:26 warning "token pasting of ',' and __VA_ARGS__ is a GNU extension"
//@ diag: 11:28 warning "token pasting of ',' and __VA_ARGS__ is a GNU extension"
#define F(fmt, ...) g(fmt, ## __VA_ARGS__)
#define W(x) x
#define U(a, ...) a , ## __VA_ARGS__
int g(int,...);
int a = F(1);
int b = F(1);
int c = W(F(1, 2));
