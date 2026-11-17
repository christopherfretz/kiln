// [expr.prim.id.qual]/1, [temp.alias]/2: an alias template specialization
// before :: must name a class or an enumeration, also with dependent
// arguments when the alias does not depend on them.
// Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:22 error "type 'A<X>' (aka 'int') cannot be used prior to '::' because it has no members"
//@ diag: 13:20 note "type alias template 'A' declared here"
//@ diag: 15:18 error "type 'A<char>' (aka 'int') cannot be used prior to '::' because it has no members"
//@ diag: 13:20 note "type alias template 'A' declared here"
//@ diag: 16:54 error "type 'A<X>' (aka 'int') cannot be used prior to '::' because it has no members"
//@ diag: 13:20 note "type alias template 'A' declared here"
template<typename> using A = int;
template<typename X> A<X>::a;
int f() { return A<char>::c; }
template<class X> struct S { static int g() { return A<X>::d; } };
int h() { return S<int>::g(); }
