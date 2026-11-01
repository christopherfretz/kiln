// [dcl.attr.fallthrough] (WP-217): -Wimplicit-fallthrough for labels
// nested in statements: Duff's device (labels in a `do` body), and labels
// in an `if` branch or a `while`/`for` body, entered from the condition
// (an unannotated predecessor unless the condition prunes the edge).
//@ args: -Wimplicit-fallthrough
//@ strict-diags
//@ diag: 64:3 warning "unannotated fall-through between switch labels"
//@ diag: 64:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 64:3 note "insert 'break;' to avoid fall-through"
//@ diag: 65:3 warning "unannotated fall-through between switch labels"
//@ diag: 65:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 65:3 note "insert 'break;' to avoid fall-through"
//@ diag: 66:3 warning "unannotated fall-through between switch labels"
//@ diag: 66:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 66:3 note "insert 'break;' to avoid fall-through"
//@ diag: 71:3 warning "unannotated fall-through between switch labels"
//@ diag: 71:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 71:3 note "insert 'break;' to avoid fall-through"
//@ diag: 73:3 warning "unannotated fall-through between switch labels"
//@ diag: 73:3 note "insert 'break;' to avoid fall-through"
//@ diag: 79:3 warning "unannotated fall-through between switch labels"
//@ diag: 79:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 79:3 note "insert 'break;' to avoid fall-through"
//@ diag: 81:3 warning "unannotated fall-through between switch labels"
//@ diag: 81:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 81:3 note "insert 'break;' to avoid fall-through"
//@ diag: 82:3 warning "unannotated fall-through between switch labels"
//@ diag: 82:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 82:3 note "insert 'break;' to avoid fall-through"
//@ diag: 83:3 warning "unannotated fall-through between switch labels"
//@ diag: 83:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 83:3 note "insert 'break;' to avoid fall-through"
//@ diag: 83:22 warning "unannotated fall-through between switch labels"
//@ diag: 83:22 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 83:22 note "insert 'break;' to avoid fall-through"
//@ diag: 84:3 warning "unannotated fall-through between switch labels"
//@ diag: 84:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 84:3 note "insert 'break;' to avoid fall-through"
//@ diag: 85:3 warning "unannotated fall-through between switch labels"
//@ diag: 85:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 85:3 note "insert 'break;' to avoid fall-through"
//@ diag: 86:3 warning "unannotated fall-through between switch labels"
//@ diag: 86:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 86:3 note "insert 'break;' to avoid fall-through"
//@ diag: 87:3 warning "unannotated fall-through between switch labels"
//@ diag: 87:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 87:3 note "insert 'break;' to avoid fall-through"
//@ diag: 88:3 warning "unannotated fall-through between switch labels"
//@ diag: 88:3 note "insert 'break;' to avoid fall-through"
int g(int);
void k(int x) {
  switch (x) {
  case 1: if (true) return;
  case 2: if (false) g(1); else return;
  case 3: while (true) {}
  case 4: if (x) { return; } else { return; }
  case 5: { switch (x) { case 0: return; } return; }
  case 6: g(6);
  }
}
void duff(int n, int x) {
  switch (n % 4) {
  case 0: do { g(0);
  case 3: g(3);
  case 2: g(2);
  case 1: g(1);
          } while (--n > 0);
  }
  switch (x) {
  case 0: if (n) {
  case 1: g(1);
  }
  case 2: break;
  }
}
void nest(int x, int n) {
  switch (x) {
  case 0: if (n) { g(0); } else {
  case 1: g(1);
  }
  case 2: while (n) {
  case 3: g(3); }
  case 4: for (; n;) case 5: g(5);
  case 6: if (0) { case 7: g(7); }
  case 8: if (1) { g(8); } else { case 9: g(9); }
  case 10: while (0) { case 11: g(11); }
  case 12: if (n) { L: case 13: g(13); }
  case 14: break;
  }
}
