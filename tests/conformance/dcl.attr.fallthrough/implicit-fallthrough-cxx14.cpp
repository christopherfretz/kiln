// [dcl.attr.fallthrough] (WP-217): before C++17 the -Wimplicit-fallthrough
// note suggests '[[clang::fallthrough]];', which also silences it; an
// unreachable annotation still warns under -Wunreachable-code-fallthrough
// (outside instantiations).
//@ std: c++14
//@ args: -Wimplicit-fallthrough -Wunreachable-code-fallthrough
//@ strict-diags
//@ diag: 23:3 warning "unannotated fall-through between switch labels"
//@ diag: 23:3 note "insert '[[clang::fallthrough]];' to silence this warning"
//@ diag: 23:3 note "insert 'break;' to avoid fall-through"
//@ diag: 28:3 warning "unannotated fall-through between switch labels"
//@ diag: 28:3 note "insert 'break;' to avoid fall-through"
//@ diag: 32:5 warning "fallthrough annotation in unreachable code"
//@ diag: 39:30 warning "unannotated fall-through between switch labels"
//@ diag: 41:15 note "in instantiation of function template specialization 't<int>' requested here"
//@ diag: 39:30 note "insert '[[clang::fallthrough]];' to silence this warning"
//@ diag: 39:30 note "insert 'break;' to avoid fall-through"
int g(int);
void f(int x) {
  switch (x) {
  case 0:
    g(0);
  case 1:
    g(1);
    [[clang::fallthrough]];
  case 2:
    g(2);
  case 3:
    break;
  case 4:
    return;
    [[clang::fallthrough]];
  case 5:
  default:
    ;
  }
}
template <class T> void t(T x) {
  switch (x) { case 1: g(1); case 2: return; [[clang::fallthrough]]; case 3: break; }
}
template void t<int>(int);
