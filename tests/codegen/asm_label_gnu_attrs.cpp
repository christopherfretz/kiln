// [dcl.asm] GNU asm labels with GNU attributes (WP-384, clang
// `ParseDeclGroup` / `ParseAsmAttributesAfterDeclarator`): the first
// function declarator of a group takes `__attribute__((...))` before its
// asm label as well as after it; the label names the symbol (`@lbl`,
// `@lbl2`, `@lbl3`).
void f() __attribute__((noinline)) asm("lbl");
void g() asm("lbl2") __attribute__((noinline));
void k() __attribute__((cold)) asm("lbl3") __attribute__((noinline));
void f() {}
void g() {}
void k() {}
int h() {
  f();
  g();
  k();
  return 0;
}
