// [temp.deduct.general]/8, [temp.inst]/11: a member function template's
// specialization whose signature fails to substitute is remembered as
// failed. Formed first by a SFINAE probe (`p<E>` picks its other
// overload), and then needed by a call in a member function's body that
// is instantiated, it is diagnosed at that call exactly as when the call
// comes first (`F`, no probe before; WP-3283, spec/sema-templates.md
// §5.7.3 "Hard use of a silent failure"). The call is deduced again, so
// the failure is the deduction's substitution failure, with its reason.
//@ expect: compile-fail
//@ strict-diags
//@ oracle: gcc
//@ diag: 23:36 error "no matching function for call to 'ok'"
//@ diag: 28:15 note "in instantiation of member function 'S<E>::use' requested here"
//@ diag: 22:25 note "candidate template ignored: substitution failure [with A = <int>, B = int]: invalid operands to binary expression ('E' and 'int')"
//@ diag: 23:36 error "no matching function for call to 'ok'"
//@ diag: 29:15 note "in instantiation of member function 'S<F>::use' requested here"
//@ diag: 22:25 note "candidate template ignored: substitution failure [with A = <int>, B = int]: invalid operands to binary expression ('F' and 'int')"
struct E {};
struct F {};
template <class T> struct S {
  template <class... A, class B>
  static constexpr auto ok(B) -> decltype(T() + B()) { return {}; }
  static int use() { return sizeof(ok<int>(0)); }
};
template <class U> auto p(int) -> decltype(S<U>::template ok<int>(0)) { return 0; }
template <class U> char p(long) { return 0; }
static_assert(sizeof(p<E>(0)) == 1);
int e = S<E>::use();
int f = S<F>::use();
// After the call failed, the probe still fails quietly.
static_assert(sizeof(p<F>(0)) == 1);
