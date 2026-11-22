// [dcl.fct.def.general]/1, [dcl.decl.general]/1, [diff.cpp03.lex]
// (`noexcept` is an identifier before C++11): at namespace scope, a
// function declarator followed by a token that neither continues the
// declaration (`;`, `,`, `=`, an asm label, GNU attributes), nor starts
// a function body, nor starts another declaration after a missing `;`
// is "expected function body after function declarator" (clang
// `ParseDeclGroup`, `err_expected_fn_body`), and the declaration is
// skipped through its `;` (here the body and the next declaration):
// `void C::f(int) noexcept;` declares nothing, so no out-of-line member
// error follows. Before a decl-specifier the
// `;` is missing instead. WP-1849. Verified against clang 22 and 19.
//@ std: c++98
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:16 error "expected function body after function declarator"
//@ diag: 24:10 error "expected function body after function declarator"
//@ diag: 25:11 error "expected function body after function declarator"
//@ diag: 26:25 error "expected function body after function declarator"
//@ diag: 27:10 error "expected ';' after top level declarator"
//@ diag: 29:20 error "no member named 'h3' in namespace 'N'"
//@ diag: 30:13 error "expected function body after function declarator"
struct C { void f(int); void g(); };
void C::f(int) noexcept;
void h() noexcept;
void h2() foo;
namespace N { void h3() noexcept; }
void h4() int x;
int k = sizeof(x);
int k2 = sizeof(N::h3);
void C::g() noexcept {}
int last;
