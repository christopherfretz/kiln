// [dcl.attr.grammar]/6: an attribute-namespace is matched by its reserved
// spelling too, as GCC and clang do: `__gnu__` names `gnu` and `_Clang`
// names `clang`. libstdc++ spells its attributes `[[__gnu__::__x__]]`
// (bits/allocator.h `[[__gnu__::__always_inline__]]`). Verified against
// clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:18 warning "'old' is deprecated: old"
//@ diag: 16:3 note "'old' has been explicitly marked deprecated here"
//@ diag: 24:3 warning "unannotated fall-through between switch labels"
//@ diag: 24:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 24:3 note "insert 'break;' to avoid fall-through"
//@ args: -Wimplicit-fallthrough
[[__gnu__::__unused__]] static int x;
[[__gnu__::__noinline__]] int g();
[[__gnu__::__deprecated__("old")]] int old();
int f(int v) {
  switch (v) {
  case 0:
    ++v;
    [[_Clang::fallthrough]];
  case 1: return old();
  case 2: ++v;
  case 3: return v;
  }
  return 0;
}
