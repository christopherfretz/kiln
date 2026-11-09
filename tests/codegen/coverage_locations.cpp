// Coverage region bounds that depend on expression locations (WP-1083,
// spec/codegen.md §5.4 "Instrumentation"): a qualified base initializer
// `N::B(4)` starts at its nested-name-specifier (clang: the
// `ElaboratedTypeLoc`'s begin), and a scalar compound literal
// `(long){}` ends at its `}` (clang: its `InitListExpr`'s).
//@ args: -fprofile-instr-generate -fcoverage-mapping -fcoverage-compilation-dir=/cov -Xclang -disable-llvm-passes
namespace N { struct B { B(int) {} }; }
struct D : N::B {
  D(int x) : N::B(x ? 4 : 5) {}
};
template <class T> struct Base { Base(int) {} };
template <class T> struct E : Base<T> {
  E() : E::Base(1) {}
};
long g(int x) {
  D d(x);
  E<int> e;
  long r = 0;
  if (x)
    r = (long){};
  else
    r = (long){2};
  if ((long){x})
    return 1;
  return r + (x ? (long){} : (long){1});
}
