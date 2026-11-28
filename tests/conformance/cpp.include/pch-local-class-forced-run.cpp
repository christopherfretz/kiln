// [class.local], spec/codegen.md §2a (WP-3687): an inline function with a
// local-class `used` or `constructor` member in a header included via PCH is
// emitted (`linkonce_odr`) in the consumer even when the enclosing inline
// function is never called. Covered by `--pch-diff` replay and snapshot lanes.
//@ expect: run
//@ std: c++20
#include "Inputs/local_class_forced.h"

extern "C" void _ZZ8pch_usedvEN1L1cEv();

int main() {
  _ZZ8pch_usedvEN1L1cEv();
  return (pch_used_ran == 1 && pch_ctor_ran == 1) ? 0 : 1;
}
