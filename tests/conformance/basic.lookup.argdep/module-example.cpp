// [basic.lookup.argdep]/4-5: the standard's example (translation unit #3).
// ADL finds an exported declaration of named module M that is not visible
// only when it shares its innermost enclosing non-inline namespace with an
// associated entity attached to M: R::f is found through R::X, S::f is not
// although S is an associated namespace (through S::Z). In the
// instantiation of apply the instantiation context makes S::f visible, but
// R::g has internal linkage in another unit (TU #2) and is not found; that
// error is located in Inputs/adl-n.cppm (checked by kiln_driver's
// `module_adl_example` test). R::f is not visible to qualified lookup.
//@ expect: compile-fail
//@ std: c++20
//@ args: -fmodule-file=M=%{test_dir}/Inputs/adl-m.cppm -fmodule-file=N=%{test_dir}/Inputs/adl-n.cppm -fmodule-file=Q=%{test_dir}/Inputs/adl-q.cppm
//@ skip(clang): clang needs a two-step --precompile build of the Inputs modules that the harness cannot drive
//@ strict-diags
//@ diag: 25:3 error "no matching function for call to 'f'"
//@ diag: 31:6 error "no type named 'f' in namespace 'R'"
module Q;
import N;
namespace S {
  struct Z { template<typename T> operator T(); };
}
void test() {
  auto x = make();              // OK, decltype(x) is R::X in module M
  f(x);                         // OK, calls R::f from interface of M
  f(x, S::Z());                 // error: S::f in module M not considered
                                // even though S is an associated namespace
  apply(x, S::Z());             // error: S::f is visible in instantiation context, but
                                // R::g has internal linkage and cannot be used outside TU #2
  // Last, since recovery declares a variable x: R and R::f are not
  // visible here.
  R::f(x);
}
