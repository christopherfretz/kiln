// [dcl.asm], GNU __extension__ (spec/parse.md §4): only at namespace scope
// does __extension__ precede an asm-declaration; in a class it is a
// decl-specifier and in a block the unary operator, so asm there is an
// error, as clang.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:26 error "expected member name or ';' after declaration specifiers"
//@ diag: 10:26 error "expected unqualified-id"
struct S { __extension__ asm("nop"); };
void f() { __extension__ asm("nop"); }
