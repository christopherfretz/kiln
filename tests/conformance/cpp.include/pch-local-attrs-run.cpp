// [dcl.attr.grammar]/5, [dcl.attr.noreturn], [basic.link]/6: attributes on
// block-scope declarations in an included header (a GNU-attributed
// block-scope function redeclaration as in libstdc++'s
// <bits/c++config.h>, `[[maybe_unused]]` locals) apply as written. With
// `--pch-diff`, the header's precompiled image carries them (WP-1310).
//@ expect: run
#include "Inputs/local_attrs.h"

namespace la {
void stop(int) noexcept { __builtin_abort(); }
}

int main() {
  la::call_stop(0);
  return la::locals() == 12 ? 0 : 1;
}
