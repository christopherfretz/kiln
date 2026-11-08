// `-fsanitize=hwaddress` (frontend output): `sanitize_hwaddress` and the
// `no_sanitize_hwaddress` global metadata.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.2, WP-675.
//@ expect: ir
//@ std: c++20
//@ requires: target-aarch64
//@ args: -fsanitize=hwaddress -Xclang -disable-llvm-passes
#include "Inputs/san-attrs.h"
