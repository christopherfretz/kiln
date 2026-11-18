// [class.access]/4, [over.over]/3, [temp.arg.explicit]/4: a member
// template-id with no target type (`(void)&V::h<int>;`, `sizeof`,
// `decltype`) names its one specialization, and the member's name is
// access-checked as in any other use. clang 22 resolves such an operand
// without the access check (ResolveSingleFunctionTemplateSpecialization)
// and accepts all of the errors below; GCC 16.1 rejects them as kiln does
// (docs/status/deviations.md). With a target type, called, or named by a
// friend or a member, clang and kiln agree. Found running GCC's C++
// testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not access-check a template-id resolved without a target
//@ diag: 30:23 error "'h' is a private member of 'V'"
//@ diag: 24:34 note "implicitly declared private here"
//@ diag: 31:23 error "'m' is a private member of 'V'"
//@ diag: 25:27 note "implicitly declared private here"
//@ diag: 32:22 error "'h' is a private member of 'V'"
//@ diag: 24:34 note "implicitly declared private here"
//@ diag: 33:30 error "'h' is a private member of 'V'"
//@ diag: 24:34 note "implicitly declared private here"
//@ diag: 34:25 error "'h' is a private member of 'V'"
//@ diag: 24:34 note "implicitly declared private here"
class V {
  template <class U> static void h(U) {}
  template <class U> void m(U) {}
  friend void fr();
  void self() { (void)&V::h<int>; (void)&V::m<int>; (void)sizeof(&V::h<int>); }
};
void fr() { (void)&V::h<int>; (void)&V::m<int>; using T = decltype(&V::h<int>); }
void g1() { (void)&V::h<int>; }
void g2() { (void)&V::m<int>; }
void g3() { (void)V::h<int>; }
void g4() { (void)sizeof(&V::h<int>); }
using T5 = decltype(&V::h<int>);
