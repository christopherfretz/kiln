// [dcl.attr.fallthrough]/1 note (WP-217): -Wimplicit-fallthrough (off by
// default, clang `warn_unannotated_fallthrough`): a case or default label
// reached from a preceding statement without a [[fallthrough]] annotation.
// Notes suggest '[[fallthrough]];' (unless some path into the label is
// annotated) and 'break;'. -Wunreachable-code-fallthrough flags an
// annotation in unreachable code. Templates are checked per instantiation.
//@ args: -Wimplicit-fallthrough -Wunreachable-code-fallthrough
//@ strict-diags
//@ diag: 100:3 warning "unannotated fall-through between switch labels"
//@ diag: 100:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 100:3 note "insert 'break;' to avoid fall-through"
//@ diag: 106:3 warning "unannotated fall-through between switch labels"
//@ diag: 106:3 note "insert 'break;' to avoid fall-through"
//@ diag: 110:3 warning "unannotated fall-through between switch labels"
//@ diag: 110:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 110:3 note "insert 'break;' to avoid fall-through"
//@ diag: 114:3 warning "unannotated fall-through between switch labels"
//@ diag: 116:5 warning "fallthrough annotation in unreachable code"
//@ diag: 121:3 warning "unannotated fall-through between switch labels"
//@ diag: 121:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 121:3 note "insert 'break;' to avoid fall-through"
//@ diag: 123:3 warning "unannotated fall-through between switch labels"
//@ diag: 123:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 123:3 note "insert 'break;' to avoid fall-through"
//@ diag: 126:3 warning "unannotated fall-through between switch labels"
//@ diag: 126:3 note "insert 'break;' to avoid fall-through"
//@ diag: 133:3 warning "unannotated fall-through between switch labels"
//@ diag: 133:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 133:3 note "insert 'break;' to avoid fall-through"
//@ diag: 140:3 warning "unannotated fall-through between switch labels"
//@ diag: 140:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 140:3 note "insert 'break;' to avoid fall-through"
//@ diag: 152:3 warning "unannotated fall-through between switch labels"
//@ diag: 152:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 152:3 note "insert 'break;' to avoid fall-through"
//@ diag: 157:3 warning "unannotated fall-through between switch labels"
//@ diag: 157:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 157:3 note "insert 'break;' to avoid fall-through"
//@ diag: 161:30 warning "unannotated fall-through between switch labels"
//@ diag: 163:15 note "in instantiation of function template specialization 't<int>' requested here"
//@ diag: 161:30 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 161:30 note "insert 'break;' to avoid fall-through"
//@ diag: 161:30 warning "unannotated fall-through between switch labels"
//@ diag: 164:15 note "in instantiation of function template specialization 't<long>' requested here"
//@ diag: 161:30 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 161:30 note "insert 'break;' to avoid fall-through"
//@ diag: 166:30 warning "unannotated fall-through between switch labels"
//@ diag: 168:15 note "in instantiation of function template specialization 't1<int>' requested here"
//@ diag: 166:30 note "insert 'break;' to avoid fall-through"
//@ diag: 172:30 warning "unannotated fall-through between switch labels"
//@ diag: 170:15 note "expanded from macro 'CASE2'"
//@ diag: 177:3 warning "unannotated fall-through between switch labels"
//@ diag: 177:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 177:3 note "insert 'break;' to avoid fall-through"
//@ diag: 179:3 warning "unannotated fall-through between switch labels"
//@ diag: 179:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 179:3 note "insert 'break;' to avoid fall-through"
//@ diag: 180:3 warning "unannotated fall-through between switch labels"
//@ diag: 180:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 180:3 note "insert 'break;' to avoid fall-through"
//@ diag: 181:3 warning "unannotated fall-through between switch labels"
//@ diag: 181:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 181:3 note "insert 'break;' to avoid fall-through"
//@ diag: 185:51 warning "unannotated fall-through between switch labels"
//@ diag: 185:51 note "insert 'break;' to avoid fall-through"
//@ diag: 188:3 warning "unannotated fall-through between switch labels"
//@ diag: 188:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 188:3 note "insert 'break;' to avoid fall-through"
//@ diag: 189:3 warning "unannotated fall-through between switch labels"
//@ diag: 189:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 189:3 note "insert 'break;' to avoid fall-through"
//@ diag: 191:3 warning "unannotated fall-through between switch labels"
//@ diag: 191:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 191:3 note "insert 'break;' to avoid fall-through"
//@ diag: 195:3 warning "unannotated fall-through between switch labels"
//@ diag: 195:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 195:3 note "insert 'break;' to avoid fall-through"
//@ diag: 202:5 warning "unannotated fall-through between switch labels"
//@ diag: 202:5 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 202:5 note "insert 'break;' to avoid fall-through"
//@ diag: 205:31 warning "unannotated fall-through between switch labels"
//@ diag: 205:31 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 205:31 note "insert 'break;' to avoid fall-through"
//@ diag: 206:46 warning "unannotated fall-through between switch labels"
//@ diag: 206:46 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 206:46 note "insert 'break;' to avoid fall-through"
//@ diag: 212:3 warning "unannotated fall-through between switch labels"
//@ diag: 212:3 note "insert '[[fallthrough]];' to silence this warning"
//@ diag: 212:3 note "insert 'break;' to avoid fall-through"
//@ diag: 166:30 warning "unannotated fall-through between switch labels"
//@ diag: 169:13 note "in instantiation of function template specialization 't1<long>' requested here"
//@ diag: 166:30 note "insert 'break;' to avoid fall-through"
int g(int);
[[noreturn]] void die();
#define CASE3 case 3
void f(int x, int y) {
  switch (x) {
  case 0:
    g(0);
  case 1:
  case 2:
    g(2);
    [[fallthrough]];
  CASE3:
    g(3);
  case 4:
    break;
  case 5:
    if (y) break;
  case 6:
    if (y) { g(1); [[fallthrough]]; } else return;
  case 7:
    if (y) [[fallthrough]];
  case 8:
    return;
    [[fallthrough]];
  case 9:
    die();
  case 10:
    ;
  case 11:
    g(11);
  case 12: {
    g(12);
  }
  case 13:
  case 14:
    break;
  case 15:
    for (;;) {}
  case 16:
    while (y) g(1);
  default:
    g(0);
  }
  switch (y) {
  case 1: {
    switch (x) { case 1: g(1); }
  }
  case 2:
    g(2);
    break;
  case 3:
    return;
    g(4);
  case 4:
    g(4);
  }
  switch (x) {
  case 1:
    if (0) { g(1); }
  case 2:
    g(1); goto L;
  case 3:
  L:
    g(2);
  case 4: ;
  }
}
template <class T> void t(T x) {
  switch (x) { case 1: g(1); case 2: return; [[fallthrough]]; case 3: break; }
}
template void t<int>(int);
template void t<long>(long);
template <class T> void t1(T x) {
  switch (x) { case 1: g(1); case 2: break; }
}
template void t1<int>(int);
void u1() { t1<long>(2); }
#define CASE2 case 2
void m1(int x) {
  switch (x) { case 1: g(1); CASE2: break; }
}
void n1(int x, int y) {
  switch (x) {
  case 1: { g(1); }
  case 2: { g(2); break; }
  case 3: do { g(3); } while (y);
  case 4: try { g(4); } catch (...) { }
  case 5: if (y) { die(); } else { g(5); }
  default: g(6);
  }
}
void n2(int x, int y) {
  auto l = [](int z) { switch (z) { case 1: g(1); case 2: break; } };
  switch (x) {
  case 1: for (int i = 0; i < y; ++i) g(i);
  case 2: if constexpr (true) { g(2); } else { g(3); }
  case 3: g(3); [[fallthrough]];
  case 4: lbl: g(4);
  case 5: goto lbl;
  case 6: switch (y) { case 0: return; default: return; }
  case 7: while (true) { g(7); }
  case 8: { int q = y; (void)q; }
  case 9: g(9);
  }
}
void n3(int x) {
  switch (x) {
    case 0:
    case 1: g(1);
    default: ;
  }
  switch (x) { case 1: g(1); }
  switch (x) { default: g(0); case 1: g(1); }
  switch (x) { case 1: case 2: case 3: g(3); case 4: g(4); }
}
int n4(int x) {
  switch (x) {
  case 1: return 1; g(1);
  case 2: x++;
  case 3: throw 1;
  case 4: x--;
  }
  return x;
}
