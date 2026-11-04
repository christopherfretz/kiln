// GNU `gnu_inline` (GNU extension; [dcl.attr.grammar]/5, [dcl.inline];
// spec/sema.md "GNU inline functions", clang handleGNUInlineAttr): the
// attribute applies only to functions, is ignored with a warning on a
// function not declared `inline`, and warns when the declaration lacks
// `extern` (C++ treats such a definition as available externally anyway).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 13:16 warning "'gnu_inline' attribute requires function to be marked 'inline', attribute ignored"
//@ diag: 14:23 warning "'gnu_inline' attribute without 'extern' in C++ treated as externally available, this changed in Clang 10"
//@ diag: 15:30 warning "'gnu_inline' attribute without 'extern' in C++ treated as externally available, this changed in Clang 10"
//@ diag: 16:16 warning "'gnu_inline' attribute only applies to functions"
extern inline __attribute__((gnu_inline)) int fine(int x) { return x; }
__attribute__((gnu_inline)) int plain(int x) { return x; }
inline __attribute__((gnu_inline)) int no_extern(int x) { return x; }
static inline __attribute__((gnu_inline)) int internal(int x) { return x; }
__attribute__((gnu_inline)) int not_a_function;
int use() { return fine(1) + plain(2) + no_extern(3) + internal(4) + not_a_function; }
