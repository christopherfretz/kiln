// [dcl.attr.fallthrough], [stmt.pre]: a GNU `__attribute__((fallthrough));`
// (or `__fallthrough__`) at the start of a statement is an attributed null
// statement in every language mode, after `[[...]]` specifiers too, with
// no C++17-extension warning; it marks the fallthrough for
// -Wimplicit-fallthrough, so only the unmarked case at line 30 warns
// (clang `ParseStatementOrDeclarationAfterAttributes`, `kw___attribute`).
// A GNU attribute before a declaration still appertains to it.
//@ expect: run
//@ std: c++11
//@ args: -pedantic -Wimplicit-fallthrough -Wunused
//@ strict-diags
//@ diag: 30:3 warning "unannotated fall-through between switch labels"
//@ diag: 30:3 note "insert '[[clang::fallthrough]];' to silence this warning"
//@ diag: 30:3 note "insert 'break;' to avoid fall-through"
int f(int x) {
  switch (x) {
  case 0:
    x++;
    __attribute__((__fallthrough__));
  case 1:
    x++;
    [[]] __attribute__((fallthrough));
  case 2: {
    __attribute__((unused)) int y = x;
    x += 1;
    __attribute__((fallthrough)) __attribute__((fallthrough));
  }
  case 3:
    x++;
  case 4:
    return x;
  }
  return 0;
}
int main() { return f(0) == 4 ? 0 : 1; }
