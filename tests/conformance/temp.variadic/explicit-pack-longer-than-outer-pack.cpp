// [temp.variadic]/7, [temp.deduct.general]/2: explicitly specifying more arguments for a member
// template's pack than the enclosing class's pack expanded alongside it is a substitution failure,
// reported as clang does with '(at least N vs. M)' (the explicit pack may still grow)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:12 error "no matching function for call to 'f'"
//@ diag: 11:66 note "candidate template ignored: substitution failure [with U = <int, int, int>]: pack expansion contains parameter pack 'U' that has a different length (at least 3 vs. 2) from outer parameter packs"
//@ diag: 14:13 error "no matching function for call to 'h'"
//@ diag: 13:66 note "candidate template ignored: substitution failure [with U = <int, int, int>]: pack expansion contains parameter pack 'U' that has a different length (at least 3 vs. 2) from outer parameter packs"
template<class A, class B> struct pair {};
template<class... T> struct X { template<class... U> static void f(pair<T, U>...); };
void g() { X<int, char>::f<int, int, int>(); }
template<class... T> struct Y { template<class... U> static void h(pair<T, U>... p); };
void g2() { Y<int, char>::h<int, int, int>({}, {}, {}); }
