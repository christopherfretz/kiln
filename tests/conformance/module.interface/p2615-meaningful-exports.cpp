// [module.interface]/1-3 after P2615R1 (meaningful exports): exporting a
// static_assert, a using-directive, an empty declaration, an empty export
// block and explicit specializations / instantiations is well-formed.
//@ expect: compile-ok
//@ std: c++20
//@ requires: clang>=22
//@ strict-diags
export module M;
export namespace n {
  template <class T> int f(T) { return 0; }
  template <class T> struct S {};
}
export static_assert(sizeof(int) >= 2);
export using namespace n;
export {}
export { ; }
export template <> int n::f(char) { return 1; }
export template <> struct n::S<int> {};
export template struct n::S<long>;
