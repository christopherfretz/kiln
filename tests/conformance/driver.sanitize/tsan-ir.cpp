// `-fsanitize=thread` (frontend output): `sanitize_thread` unless a
// function opts out.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.2, WP-675.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=thread -Xclang -disable-llvm-passes
#include "Inputs/san-attrs.h"
