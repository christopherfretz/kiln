// [basic.def.odr]/5, [expr.cond]/4: `c ? A : B` with `A` and `B` `static
// const` integral members is an lvalue conditional, but neither arm is an
// odr-use: each is usable in constant expressions and the lvalue-to-rvalue
// conversion is applied to the conditional. No out-of-line definition
// exists (and the class has internal linkage), so the program links only
// if each arm is emitted as its constant rather than a load of the member
// (clang `tryEmitAsConstant`). LLVM's MemorySanitizer.cpp
// `IsSoftFloatABI ? SystemZGpEndOffset : SystemZRegSaveAreaSize`.
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 160 161 320 56 9
extern "C" int printf(const char*, ...);
namespace {
struct H {
  static const unsigned A = 56;
  static const unsigned B = 160;
  bool soft;
  unsigned get() const { return soft ? A : B; }
  unsigned add(unsigned x) const { return x + (soft ? A : B); }
};
}  // namespace
struct E {
  static const int X = 4, Y = 9;
};
unsigned mk(unsigned a, unsigned b) { return a * b; }
int pick(bool c) { return c ? E::X : E::Y; }
int main(int argc, char**) {
  H h{argc > 5};
  unsigned v = mk(h.soft ? H::A : H::B, 2);
  H s{true};
  unsigned w = s.soft ? s.A : s.B;
  printf("%u %u %u %u %d\n", h.get(), h.add(1), v, w, pick(argc > 5));
}
