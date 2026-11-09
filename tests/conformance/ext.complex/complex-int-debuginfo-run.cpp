// GNU complex integer types at `-g` (WP-910): clang describes `_Complex
// int` with the `DW_ATE_lo_user` (0x80) encoding, which LLVM's IR parser
// knows only as the number 128; kiln printed the name and the backend
// rejected the module.
//@ expect: run
//@ args: -g
//@ stdout: 3 4 7
extern "C" int printf(const char *, ...);
_Complex int gi;
_Complex long long gl;
int main() {
  _Complex int li;
  __real__ li = 3;
  __imag__ li = 4;
  gi = li;
  gl = gi;
  _Complex double d = 1.5;
  printf("%d %d %lld\n", __real__ gi, __imag__ gi, __real__ gl + __imag__ gl + (long long)__imag__ d);
}
