// spec/host-calls.md §4 (WP-2501), [temp.deduct.general]/8: a host call
// the handler fails (op 3, "boom") in a SFINAE context is a hard error at
// the immediate context, with the handler's note and the context notes,
// never a substitution failure: whether the handler answers is no
// property of the template arguments. A requires-expression
// ([expr.prim.req]/5) was silently false, a deduction and a partial
// specialization's match ([temp.spec.partial.match]/2) silently skipped
// the candidate. Each is the error an `if constexpr` condition, which is
// no SFINAE context, already gave.
//@ expect: compile-fail
//@ args: -fkiln-host-call-test-handler
//@ skip(clang): kiln's host-call builtin
//@ strict-diags
//@ diag: 36:14 error "non-type template argument is not a constant expression"
//@ diag: 36:3 note "in instantiation of requirement here"
//@ diag: 38:16 note "in instantiation of function template specialization 'has<int>' requested here"
//@ diag: 31:10 note "host call failed: boom"
//@ diag: 36:14 note "in call to 'hc(3)'"
//@ diag: 41:37 error "non-type template argument is not a constant expression"
//@ diag: 43:22 note "while substituting deduced template arguments into function template 'd' [with T = int]"
//@ diag: 31:10 note "host call failed: boom"
//@ diag: 41:37 note "in call to 'hc(3)'"
//@ diag: 47:49 error "non-type template argument is not a constant expression"
//@ diag: 48:15 note "during template argument deduction for class template partial specialization 'P<T, decltype((void)K<hc(3) + sizeof(T)>{})>' [with T = int]"
//@ diag: 31:10 note "host call failed: boom"
//@ diag: 47:49 note "in call to 'hc(3)'"
using u64 = unsigned long long;
template <u64 N> struct K {};

constexpr u64 hc(unsigned op) {
  return __builtin_kiln_host_call(op, nullptr, 0, nullptr, 0, nullptr, 0,
                                  nullptr, 0);
}

template <class T> consteval bool has() { return requires {
  typename K<hc(3) + sizeof(T)>;
}; }
static_assert(!has<int>());

// Deduction: the return type's substitution.
template <class T> auto d(int) -> K<hc(3) + sizeof(T)> * { return nullptr; }
template <class T> char d(...) { return 0; }
static_assert(sizeof(d<int>(0)) == 1);

// A partial specialization's match.
template <class T, class = void> struct P { static constexpr int v = 0; };
template <class T> struct P<T, decltype((void)K<hc(3) + sizeof(T)>{})> { static constexpr int v = 1; };
static_assert(P<int>::v == 0);
