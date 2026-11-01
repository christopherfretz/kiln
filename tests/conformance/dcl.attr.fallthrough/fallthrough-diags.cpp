//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "'fallthrough' attribute cannot be applied to a declaration"
//@ diag: 16:13 error "'fallthrough' attribute only applies to empty statements"
//@ diag: 19:5 error "fallthrough annotation is outside switch statement"
//@ diag: 22:22 error "fallthrough annotation is outside switch statement"
//@ diag: 24:41 error "fallthrough annotation is outside switch statement"
//@ diag: 25:24 error "attribute 'fallthrough' cannot have an argument list"
// [dcl.attr.fallthrough]/1 (WP-182): a fallthrough statement is a null
// statement inside a switch (clang `handleFallThroughAttr`), also in a
// template definition and not across a lambda body.
int g(int);
void f(int x) {
  switch (x) {
  case 1: { [[fallthrough]] int y = 0; }
  case 2: [[fallthrough]] g(1);
  default: [[fallthrough]];
  }
  [[fallthrough]];
}
template <class T> void t(T) {
  if (sizeof(T)) { [[fallthrough]]; }
  switch (sizeof(T)) {
  case 1: { auto l = [] { if (true) { [[fallthrough]]; } }; }
  case 2: [[fallthrough()]];
  case 3: break;
  }
}
