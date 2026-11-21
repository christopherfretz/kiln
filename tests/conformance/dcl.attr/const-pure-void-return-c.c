// GNU `const` and `pure` on a C function declaration (clang
// `Sema::CheckFunctionDeclaration`, as in C++): both on one declaration
// drops `pure`; either on a function returning `void`, with or without
// a prototype, is ignored with a warning. A pointer to function is not
// checked. spec/sema.md §4.5, WP-1838.
//@ verify
__attribute__((const, const)) void g1(void); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((pure, pure, const)) int g2(void); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}}
__attribute__((const, pure)) void g3(void); // expected-warning {{'const' attribute imposes more restrictions; 'pure' attribute ignored}} expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
__attribute__((const)) void (*fp)(void);
__attribute__((pure)) void g5(void), g6(void); // expected-warning 2 {{'pure' attribute on function returning 'void'; attribute ignored}}
int g7(void) __attribute__((const));
__attribute__((const)) void g8(); // expected-warning {{'const' attribute on function returning 'void'; attribute ignored}}
void g9(void) __attribute__((pure)); // expected-warning {{'pure' attribute on function returning 'void'; attribute ignored}}
void use(void) {
  g7(); // expected-warning {{ignoring return value of function declared with const attribute}}
  g2(); // expected-warning {{ignoring return value of function declared with const attribute}}
  g1(); g3(); g8(); g9();
}
