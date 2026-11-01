//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:5 error "cannot initialize non-class type 'int' with a parenthesized initializer list"
//@ diag: 10:12 error "cannot initialize reference type 'const int &' with a parenthesized initializer list"
//@ diag: 11:15 error "cannot initialize non-class type 'long' with a parenthesized initializer list"
// [dcl.init.general]/16.1 (WP-182): only a non-parenthesized braced-init-list
// list-initializes, so a braced list in parentheses cannot initialize a
// scalar or a reference (clang's FK_ParenthesizedListInitFor*).
int x({1});
const int& r({1});
long* p = new long({2});
int y[2][2]({1, 2}, {3});
