// [namespace.def.general]/2: a namespace-definition shall inhabit a
// namespace scope. At block scope clang reports "namespaces can only be
// defined in global or namespace scope" at the `{` and skips the body; a
// missing `{` is "expected '{'" (or "expected identifier or '{'") at the
// token in its place (WP-1745). A namespace alias stays valid there.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:15 error "namespaces can only be defined in global or namespace scope"
//@ diag: 18:13 error "namespaces can only be defined in global or namespace scope"
//@ diag: 19:18 error "namespaces can only be defined in global or namespace scope"
//@ diag: 20:30 error "namespaces can only be defined in global or namespace scope"
//@ diag: 21:14 error "expected '{'"
//@ diag: 22:12 error "expected identifier or '{'"
//@ diag: 24:24 error "namespaces can only be defined in global or namespace scope"
namespace N2 { int w = 2; }
int f() {
  namespace N { int x; }
  namespace { int y; }
  namespace A::B { struct S {}; }
  namespace [[deprecated]] D {}
  namespace E;
  namespace;
  int z = 1;
  if (z) { namespace G { } }
  namespace H = ::N2;
  return z + H::w;
}
