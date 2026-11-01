// [stmt.switch], [dcl.enum]: an enum_extensibility(open) enumeration takes any
// case value and a flag_enum one any combination of its single-bit
// enumerators' bits (no 'not in enumerated type'); -Wcovered-switch-default
// needs a closed non-flag enumeration; the missing-enumerator warnings are
// unchanged (clang ShouldDiagnoseSwitchCaseNotInEnum, isClosedNonFlag; WP-230)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wswitch-enum -Wcovered-switch-default
//@ diag: 22:38 warning "case value not in enumerated type 'F'"
//@ diag: 22:11 warning "enumeration values 'F2' and 'F4' not handled in switch"
//@ diag: 26:11 warning "enumeration value 'O2' not explicitly handled in switch"
//@ diag: 29:11 warning "enumeration value 'O2' not handled in switch"
//@ diag: 33:30 warning "case value not in enumerated type 'Cl'"
//@ diag: 33:11 warning "enumeration value 'C2' not handled in switch"
//@ diag: 40:30 warning "case value not in enumerated type 'G'"
//@ diag: 40:11 warning "4 enumeration values not handled in switch: 'G1', 'G2', 'G3'..."
enum __attribute__((flag_enum)) F { F1 = 1, F2 = 2, F4 = 4 };
enum __attribute__((enum_extensibility(open))) O { O1, O2 };
enum __attribute__((enum_extensibility(closed))) Cl { C1, C2 };
enum [[clang::flag_enum]] G { G1 = 1, G2 = 2, G3 = 3, G8 = 8 };
int f(F x) {
  switch (x) { case F1: case 3: case 8: return 1; }
  return 0;
}
int g(O x) {
  switch (x) { case O1: case 7: return 1; default: return 2; }
}
int g2(O x) {
  switch (x) { case O1: case 7: return 1; }
  return 0;
}
int h(Cl x) {
  switch (x) { case C1: case 7: return 1; }
  return 0;
}
int k(F x) {
  switch (x) { case F1: case F2: case F4: return 1; default: return 2; }
}
int m(G x) {
  switch (x) { case 11: case 16: return 1; }
  return 0;
}
int n(O x) {
  switch (x) { case O1: case O2: return 1; default: return 2; }
}
