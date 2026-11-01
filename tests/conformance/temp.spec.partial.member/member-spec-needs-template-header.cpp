// [temp.expl.spec]/17, [temp.mem]/1: a member of an explicit specialization (`Outer<int>`) is
// declared with one fewer template-parameter-list; an extra `template<class U>` header is
// rejected (as clang: "extraneous template parameter list" / "template parameter list matching")
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:1 error "template parameter list matching the non-templated nested type 'Outer<int>' should be empty ('template<>')"
//@ diag: 11:1 error "template parameter list matching the non-templated nested type 'Outer<int>' should be empty ('template<>')"
//@ diag: 12:1 error "template parameter list matching the non-templated nested type 'Outer<int>' should be empty ('template<>')"
template<class T> struct Outer { template<class U> struct Inner; struct N; template<class U> void f(); };
template<class U> struct Outer<int>::N {};
template<class U> template<class V> struct Outer<int>::Inner<V*> {};
template<class U> template<class V> void Outer<int>::f() {}
