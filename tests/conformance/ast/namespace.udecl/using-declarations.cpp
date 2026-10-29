//@ expect: ast
// [namespace.udecl]: a using-declaration introduces each declaration it
// names (a UsingShadowDecl, implicit) into the declarative region where it
// appears; a reference found through it records the shadow.
namespace N {
int x;
int twice(int n) { return n * 2; }
typedef int I;
struct S { int m; };
namespace M { int z; }
}
using N::x;
using N::twice, N::I, N::S;
using ::N::M::z;
int a = x + twice(z);
I b = 2;
S *p;
int g() {
  using N::M::z;
  return z;
}
