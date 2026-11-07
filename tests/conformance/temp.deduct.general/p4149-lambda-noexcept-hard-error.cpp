// [temp.deduct.general]/8 (P4149R1, Croydon DR, item 2): the
// noexcept-specifier of a non-generic lambda in a function template's
// signature is instantiated with the closure type but is outside the
// immediate context, so an invalid one is a hard error even though a
// fallback overload exists.
// No oracle: clang 19/22 and GCC 16.1 predate the DR and still treat it
// as a substitution failure (they select `long foo(...)`); the expectation
// follows the wording.
//@ expect: compile-fail
//@ skip(clang): clang 22 predates P4149R1 item 2 and accepts
//@ strict-diags
//@ diag: 17:71 error "invalid operands to binary expression ('X' and 'int')"
//@ diag: 17:44 note "in instantiation of exception specification for 'operator()' requested here"
//@ diag: 19:10 note "while substituting deduced template arguments into function template 'foo' [with T = X]"
//
struct X {};
template <class T> auto foo(T) -> decltype([]() noexcept(noexcept(T() + 1)) { return 0; }());
long foo(...);
auto r = foo(X{});
