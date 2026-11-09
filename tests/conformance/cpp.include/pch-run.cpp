// [cpp.include]: a test compiled and linked against a precompiled header
// (-include-pch) runs with the header's definitions (WP-1023).
//@ expect: run
//@ pch: Inputs/pch_unguarded.h
int main() {
  PchPoint p{pch_value, PCH_MACRO(1)};
  return p.sum() == 45 ? 0 : 1;
}
