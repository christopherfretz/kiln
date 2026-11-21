// The x86 `interrupt` attribute (clang `AnyX86InterruptAttr`,
// `addX86InterruptAttrs`): a handler's definition is `x86_intrcc` with
// a `byval` first parameter of the pointee's IR type (`i8` for `void`,
// `type opaque` for a class never defined, `i1` for `bool`), and
// `"disable-tail-calls"="true"`; it is implicitly `used` (in
// `llvm.compiler.used`, so an inline one is emitted); a declaration
// keeps the C calling convention. Arguments of the attribute are
// ignored. spec/sema.md §4.5, spec/abi.md, WP-1745.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu
struct frame;
struct full { unsigned long ip, cs, flags, sp, ss; };
typedef unsigned long uword;
__attribute__((interrupt)) void h1(frame *f) {}
__attribute__((interrupt)) void h2(full *f, uword code) { (void)f; (void)code; }
[[gnu::interrupt]] void h3(void *f) {}
__attribute__((interrupt)) void h4(int *f, uword c);
__attribute__((interrupt)) void h5(bool *f);
void h5(bool *f) {}
__attribute__((interrupt)) static void h6(frame *f) {}
__attribute__((interrupt)) inline void h7(frame *f) {}
__attribute__((interrupt)) void h8(frame *f, unsigned long long c, ...) {}
__attribute__((interrupt(1))) void h9(frame *f) {}
struct S { __attribute__((interrupt)) static void sm(frame *f) {} };
template <class T> __attribute__((interrupt)) void ht(T *f) {}
template void ht<full>(full *);
void (*hp)(int *, uword) = h4;
void use(bool b) {
  void (*p)(frame *) = b ? h1 : h6;
  p(nullptr);
  hp(nullptr, 0);
}
