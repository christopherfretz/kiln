// [temp.concept], [temp.pre]/2: a concept is no member template; one
// declared in a class template is rejected in its definition and the
// specialization declares nothing for it (the -run sibling instantiates
// each kind of member template).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 11:30 error "concept declarations may only appear in global or namespace scope"

template <class T> struct A {
  template <class U> concept C = sizeof(U) == sizeof(T);
  T t;
};
A<int> a;
