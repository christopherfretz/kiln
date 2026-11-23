// [temp.inst]/3, [temp.deduct.general]/8: a value-dependent
// `align_value(N)` on a function template's parameter is checked when
// the template is instantiated (clang
// `instantiateDependentAlignValueAttr`); an `N` that is not a power of
// two is a substitution failure. `align_value` applies to variables and
// typedefs, not to a non-static data member (clang subjects `Var,
// TypedefName`). spec/sema.md §4.5 "Parameter attributes", WP-2476.
//@ verify
template <int N> void f(int *p __attribute__((align_value(N)))) {} // expected-note {{candidate template ignored: substitution failure [with N = 3]: requested alignment is not a power of 2}}
template <int N> struct S { int *q __attribute__((align_value(N))); }; // expected-warning {{'align_value' attribute only applies to variables and typedefs}}
struct F { int *q __attribute__((align_value(16))); }; // expected-warning {{'align_value' attribute only applies to variables and typedefs}}
void u() {
  f<16>(0);
  f<3>(0); // expected-error {{no matching function for call to 'f'}}
  S<5> s;
}
