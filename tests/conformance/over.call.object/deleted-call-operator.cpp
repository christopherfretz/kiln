// [over.call.object]/1, [dcl.fct.def.delete]/2: a call through an object
// whose selected operator() is deleted is ill-formed (clang: "call to deleted
// function call operator"); a requires-clause naming a deleted member
// function is not satisfied, and the satisfaction note says why (libc++
// ranges::begin on an array of incomplete type / a deleted begin()).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:3 error "call to deleted function call operator in type 'const Begin'"
//@ diag: 20:27 note "candidate function [with T = Incomplete] has been explicitly deleted"
//@ diag: 21:27 note "candidate template ignored: constraints not satisfied [with T = Incomplete[2]]"
//@ diag: 21:71 note "because 't.begin()' would be invalid: member reference base type 'Incomplete[2]' is not a structure or union"
//@ diag: 26:8 error "attempt to use a deleted function"
//@ diag: 20:27 note "'operator()<Incomplete>' has been explicitly marked deleted here"
//@ diag: 27:3 error "no matching function for call to object of type 'const Begin'"
//@ diag: 20:27 note "candidate template ignored: could not match 'T[2]' against 'R'"
//@ diag: 21:27 note "candidate template ignored: constraints not satisfied [with T = R]"
//@ diag: 21:72 note "because 't.begin()' would be invalid: attempt to use a deleted function"
struct Incomplete;
struct Begin {
  template <class T> void operator()(T (&)[2]) const = delete;
  template <class T> void operator()(T &t) const requires requires { t.begin(); } { t.begin(); }
};
inline constexpr Begin begin{};
struct R { int *begin() = delete; };
void f(Incomplete (&a)[2], R &r) {
  begin(a);
  begin(r);
}
