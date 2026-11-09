// [basic.start.main]/3: `main` shall not be named by an expression; clang
// accepts it as an extension, reported under -pedantic (-Wmain) at each
// use, a call or an address (WP-1040).
//@ std: c++26
//@ args: -pedantic
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:61 warning "referring to 'main' within an expression is a Clang extension"
//@ diag: 12:12 warning "referring to 'main' within an expression is a Clang extension"
int main();
unsigned long g() { return reinterpret_cast<unsigned long>(&main); }
void f() { main(); }
namespace N { int main(); void h() { main(); } }
