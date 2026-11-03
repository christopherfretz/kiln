// [temp.friend]/9: a friend function template whose constraint names the
// enclosing class template's parameter declares a function template of its
// own, distinct from a namespace scope template whose head is equivalent
// after substitution; a call that matches both is ambiguous.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 18:31 error "call to 'g' is ambiguous"
//@ diag: 14:33 note "candidate function [with U = int]"
//@ diag: 16:24 note "candidate function [with U = int]"
template <class T> concept C = sizeof(T) > 0;

template <class T> struct S {
  template <class U> friend int g(U, S) requires C<T> { return 1; }
};
template <class U> int g(U, S<int>) requires C<int> { return 2; }

int main() { S<int> s; return g(0, s); }
