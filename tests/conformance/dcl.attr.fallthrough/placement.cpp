// [dcl.attr.fallthrough]/1 (WP-203): a fallthrough statement must
// directly precede a switch label (clang `DiagnoseSwitchLabelsFallthrough`,
// after the body's return-type check, and only once no error occurred):
// past null statements and empty blocks, out of a branch, a try block or a
// handler, out of an inner switch into the outer one's next label. One in
// unreachable code only has to end its straight-line run; every case label
// counts as reachable. An instantiation copies its fallthrough statements
// and is not checked. The errors come in clang's order (its small pointer
// set moves the last element into the hole of each valid one).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 48:16 error "fallthrough annotation does not directly precede switch label"
//@ diag: 44:16 error "fallthrough annotation does not directly precede switch label"
//@ diag: 45:22 error "fallthrough annotation does not directly precede switch label"
//@ diag: 46:16 error "fallthrough annotation does not directly precede switch label"
//@ diag: 47:19 error "fallthrough annotation does not directly precede switch label"
//@ diag: 47:42 error "fallthrough annotation does not directly precede switch label"
int g(int);
void valid(int x) {
  switch (x) {
  case 0: if (x) { x++; [[fallthrough]]; } else { [[fallthrough]]; }
  case 1: if (0) { [[fallthrough]]; x++; }
  case 2: try { x++; [[fallthrough]]; } catch (...) { [[fallthrough]]; }
  case 3: x++; goto l; l: [[fallthrough]];
  case 4: do { x++; [[fallthrough]]; case 5: x--; } while (x);
  case 6: { int y = 0; [[fallthrough]]; }
  case 7: { if (x) [[fallthrough]]; }
  case 8: x++; [[fallthrough]]; ; {}
  case 9: { switch (x) { case 0: x++; [[fallthrough]]; } }
  case 11: x++; [[fallthrough]];
  [[likely]] case 12: x++; [[fallthrough]]; [[unlikely]] {}
  case 10: return; [[fallthrough]]; x++;
  default: break;
  }
}
template <class T> int inst(T x) {
  switch (x) { case 0: [[fallthrough]]; x++; case 1: break; }
  return 0;
}
int use = inst(1);
void invalid(int x) {
  switch (x) {
  case 0: return; [[fallthrough]]; x++;
  case 1: x++; [[fallthrough]]; break;
  case 2: for (;;) { [[fallthrough]]; }
  case 3: x++; [[fallthrough]]; l: case 4: x++;
  case 5: return; [[fallthrough]]; g(x); [[fallthrough]]; break;
  case 6: x++; [[fallthrough]];
  }
}
int after(int x) { switch (x) { case 1: [[fallthrough]]; } }
