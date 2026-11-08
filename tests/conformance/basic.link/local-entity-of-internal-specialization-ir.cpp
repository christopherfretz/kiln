// [basic.link]/8 (and /4.x, /5): a local class or closure type of a
// function template specialization takes that specialization's linkage
// (clang `getLVForLocalDecl`). `call<A>` with `A` in an unnamed namespace
// is internal (unique-external), so the lambda's call operator, the local
// class's member and the static local are internal as well. kiln gave
// them `linkonce_odr` and a comdat: two translation units that each
// instantiated `checkDataflow<(anonymous)::Analysis>` (clang's
// ClangAnalysisFlowSensitiveTests) had their lambdas merged by the
// linker, and one TU's `std::function` ran the other's lambda.
//@ expect: ir
//@ requires: clang>=22
namespace {
struct A {
  int v() { return 1; }
};
} // namespace

template <class T> int call() {
  struct Local {
    static int f() { return T().v() + 1; }
  };
  return [] { return T().v(); }() + Local::f();
}

template <class T> int *counter() {
  static int x = T().v();
  return &x;
}

static inline int helper() {
  struct S {
    static int g() { return 3; }
  };
  return S::g() + [] { return 4; }();
}

int use() { return call<A>() + *counter<A>() + helper(); }
