// [temp.explicit]/10: the explicit instantiation declaration comes from a
// precompiled header, the definition, in a reopened namespace, from this
// file: each member is defined once (one body per function, which `-g`
// checks through the backend's verifier), and the explicitly specialized
// member keeps its own body (spec/landed/codegen/WP-1425.md).
//@ expect: run
//@ args: -g
//@ pch: Inputs/extern_template_decl.h
//@ stdout: spec 11 2
namespace n { template struct A<long>; }
namespace m { int z() { return 9; } }
template int n::A<int>::f();
int main() {
  n::A<long> a;
  a.which();
  printf("%d %d\n", a.v(), n::A<int>().f());
}
