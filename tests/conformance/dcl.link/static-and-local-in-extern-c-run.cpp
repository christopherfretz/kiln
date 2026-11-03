// [dcl.link]/4-5, [basic.link]/3: inside `extern "C"` an entity with
// internal linkage has no language linkage, so two `static` variables
// or functions of one name in the `extern "C"` blocks of two namespaces
// are distinct entities (no redefinition), and each is mangled as an
// internal name; a block-scope function declaration and a friend
// declared in a class take the C language linkage of the specification
// lexically enclosing their function or class, so they name the plain C
// symbol, here reached through an asm label (WP-492).
//@ expect: run
//@ strict-diags
//@ stdout: 1 2 11 12 7 5
extern "C" int printf(const char*, ...);
extern "C" { static int twice = 1; static int sf() { return 11; } }
namespace Q { extern "C" { static int twice = 2; static int sf() { return 12; } } }
extern "C" {
  int outer() { int lf(int); return lf(6); }
  struct FS { friend int fr(); };
}
int lf_alias(int) __asm__("lf");
int lf_alias(int a) { return a + 1; }
int fr_alias() __asm__("fr");
int fr_alias() { return 5; }
int fr();
int main() {
  printf("%d %d %d %d %d %d\n", twice, Q::twice, sf(), Q::sf(), outer(), fr());
}
