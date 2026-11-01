// [temp.variadic]/7: a pack expansion mixing a pack of the class
// (already substituted, length 2) with the member template's own pack
// deduced with a different length is a substitution failure, reported
// with clang's "from outer parameter packs" text.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:11 error "no matching member function for call to 'f'"
//@ diag: 13:36 note "candidate template ignored: substitution failure [with U = <long>]: pack expansion contains parameter pack 'U' that has a different length (2 vs. 1) from outer parameter packs"
//@ diag: 17:11 error "no matching member function for call to 'f'"
//@ diag: 13:36 note "candidate template ignored: substitution failure [with U = <long, short, int>]: pack expansion contains parameter pack 'U' that has a different length (2 vs. 3) from outer parameter packs"
template <class A, class B> struct P { A a; B b; };
template <class... T> struct S {
  template <class... U> static int f(P<T, U>... ps) { return sizeof...(ps); }
};
S<int, char> s;
int a = s.f(P<int, long>{});
int b = s.f(P<int, long>{}, P<char, short>{}, P<int, int>{});
