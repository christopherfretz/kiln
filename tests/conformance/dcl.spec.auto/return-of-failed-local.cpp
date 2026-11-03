// [dcl.spec.auto.general]/12, [temp.inst]/2: a `return` whose operand
// names a local whose initializer failed to instantiate, in a function
// whose return type has a placeholder, makes the specialization invalid
// (clang: `DeduceAutoType` on an operand that `containsErrors()` is
// `AlreadyDiagnosed`), even after an earlier `return` deduced the type.
// The return type is not deduced `void` from what is left, so the caller
// gets no `void` cascade, and a later call is a substitution failure.
// With a non-placeholder return type, or in a lambda body, there is no
// second error either (spec/sema-templates.md §5.5; WP-434).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:44 error "member reference base type 'int' is not a structure or union"
//@ diag: 31:10 note "in instantiation of function template specialization 'm<int>' requested here"
//@ diag: 32:10 error "no matching function for call to 'm'"
//@ diag: 30:25 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 30:45 error "no member named 'foo' in 'S'"
//@ diag: 34:11 note "in instantiation of function template specialization 'm<S>' requested here"
//@ diag: 35:55 error "member reference base type 'int' is not a structure or union"
//@ diag: 36:10 note "in instantiation of function template specialization 'dm<int>' requested here"
//@ diag: 38:28 error "member reference base type 'int' is not a structure or union"
//@ diag: 40:19 note "in instantiation of member function 'C<int>::mm' requested here"
//@ diag: 44:13 error "member reference base type 'int' is not a structure or union"
//@ diag: 47:10 note "in instantiation of function template specialization 'first<int>' requested here"
//@ diag: 48:10 error "no matching function for call to 'first'"
//@ diag: 41:25 note "candidate template ignored: substitution failure [with T = int]"
//@ diag: 52:49 error "member reference base type 'int' is not a structure or union"
//@ diag: 53:11 note "in instantiation of function template specialization 'nested<int>' requested here"
//@ diag: 49:47 error "member reference base type 'int' is not a structure or union"
//@ diag: 50:10 note "in instantiation of function template specialization 'plain<int>' requested here"
template <class T> auto m(T t) { auto l = t.foo(); return l; }
int u1 = m(1);
int u2 = m(1);
struct S {};
auto u3 = m(S{});
template <class T> decltype(auto) dm(T t) { auto l = t.foo(); return (l); }
int u4 = dm(1);
template <class T> struct C {
  auto mm(T t) { auto l = t.foo(); return l; }
};
int u5 = C<int>().mm(1);
template <class T> auto first(T t) {
  if (t)
    return 1;
  auto l = t.foo();
  return l;
}
int u6 = first(1);
int u7 = first(1);
template <class T> int plain(T t) { auto l = t.foo(); return l; }
int u8 = plain(1);
int u9 = plain(1);
template <class T> auto nested(T t) { auto l = t.foo(); return [&] { return l; }(); }
int u10 = nested(1);
