// [temp.friend]/5, [temp.over.link]/6-7: `template <class T> template
// <C U> friend int A<T>::f(U, const B&);` befriends a member function
// template of each A<T> only when the two template heads have equivalent
// type-constraints and requires-clauses and the trailing requires-clauses
// are equivalent too (WP-1130). Denied: a type-constraint only the friend
// has (f) or only the member has (j), a leading requires-clause on one side
// (g, g2), a trailing one on one side (h, h2), an abbreviated friend
// `C auto` naming an unconstrained `auto` (i), a type-constraint naming
// another concept (m), and a type-constraint against the functionally
// equivalent requires-clause (q). Befriended: equivalent constraints of
// each form (k, r, t, i2). Of the overloads `o(U)` and `o(U) requires
// C<U>`, the friend with that trailing requires-clause names only the
// second, so `A<int>::o<char>` (the first) is denied. clang turns off
// access control for B instead (-Wunsupported-friend) and accepts the TU
// (an accepts-invalid).
//@ skip(clang): clang turns off access control for the befriending class
//@ expect: compile-fail
//@ strict-diags
//@ diag: 84:54 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 85:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 86:72 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 87:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 88:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 89:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 90:39 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 91:54 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 92:54 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 93:54 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 94:72 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 95:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 96:39 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 97:58 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 100:77 error "'n' is a private member of 'B'"
//@ diag: 117:18 note "in instantiation of function template specialization 'A<int>::f<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 101:73 error "'n' is a private member of 'B'"
//@ diag: 117:36 note "in instantiation of function template specialization 'A<int>::j<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 102:77 error "'n' is a private member of 'B'"
//@ diag: 117:54 note "in instantiation of function template specialization 'A<int>::g<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 103:92 error "'n' is a private member of 'B'"
//@ diag: 117:72 note "in instantiation of function template specialization 'A<int>::g2<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 104:77 error "'n' is a private member of 'B'"
//@ diag: 118:18 note "in instantiation of function template specialization 'A<int>::h<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 105:92 error "'n' is a private member of 'B'"
//@ diag: 118:36 note "in instantiation of function template specialization 'A<int>::h2<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 106:61 error "'n' is a private member of 'B'"
//@ diag: 118:55 note "in instantiation of function template specialization 'A<int>::i<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 107:73 error "'n' is a private member of 'B'"
//@ diag: 118:73 note "in instantiation of function template specialization 'A<int>::m<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 108:91 error "'n' is a private member of 'B'"
//@ diag: 119:18 note "in instantiation of function template specialization 'A<int>::q<int>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
//@ diag: 113:77 error "'n' is a private member of 'B'"
//@ diag: 120:55 note "in instantiation of function template specialization 'A<int>::o<char>' requested here"
//@ diag: 98:7 note "implicitly declared private here"
template <class T> concept C = sizeof(T) > 1;
template <class T> concept E = sizeof(T) > 2;
class B;
template <class T> struct A {
  template <class U> static int f(U, const B &b);
  template <C U> static int j(U, const B &b);
  template <class U> static int g(U, const B &b);
  template <class U> requires C<U> static int g2(U, const B &b);
  template <class U> static int h(U, const B &b);
  template <class U> static int h2(U, const B &b) requires C<U>;
  static int i(auto, const B &b);
  template <C U> static int m(U, const B &b);
  template <class U> requires C<U> static int q(U, const B &b);
  template <C U> static int k(U, const B &b);
  template <class U> requires C<U> static int r(U, const B &b);
  template <class U> static int t(U, const B &b) requires C<U>;
  static int i2(C auto, const B &b);
  template <class U> static int o(U, const B &b);
  template <class U> static int o(U, const B &b) requires C<U>;
};
class B {
  template <class T> template <C U> friend int A<T>::f(U, const B &);
  template <class T> template <class U> friend int A<T>::j(U, const B &);
  template <class T> template <class U> requires C<U> friend int A<T>::g(U, const B &);
  template <class T> template <class U> friend int A<T>::g2(U, const B &);
  template <class T> template <class U> friend int A<T>::h(U, const B &) requires C<U>;
  template <class T> template <class U> friend int A<T>::h2(U, const B &);
  template <class T> friend int A<T>::i(C auto, const B &);
  template <class T> template <E U> friend int A<T>::m(U, const B &);
  template <class T> template <C U> friend int A<T>::q(U, const B &);
  template <class T> template <C U> friend int A<T>::k(U, const B &);
  template <class T> template <class U> requires C<U> friend int A<T>::r(U, const B &);
  template <class T> template <class U> friend int A<T>::t(U, const B &) requires C<U>;
  template <class T> friend int A<T>::i2(C auto, const B &);
  template <class T> template <class U> friend int A<T>::o(U, const B &) requires C<U>;
  int n = 7;
};
template <class T> template <class U> int A<T>::f(U, const B &b) { return b.n; }
template <class T> template <C U> int A<T>::j(U, const B &b) { return b.n; }
template <class T> template <class U> int A<T>::g(U, const B &b) { return b.n; }
template <class T> template <class U> requires C<U> int A<T>::g2(U, const B &b) { return b.n; }
template <class T> template <class U> int A<T>::h(U, const B &b) { return b.n; }
template <class T> template <class U> int A<T>::h2(U, const B &b) requires C<U> { return b.n; }
template <class T> int A<T>::i(auto, const B &b) { return b.n; }
template <class T> template <C U> int A<T>::m(U, const B &b) { return b.n; }
template <class T> template <class U> requires C<U> int A<T>::q(U, const B &b) { return b.n; }
template <class T> template <C U> int A<T>::k(U, const B &b) { return b.n; }
template <class T> template <class U> requires C<U> int A<T>::r(U, const B &b) { return b.n; }
template <class T> template <class U> int A<T>::t(U, const B &b) requires C<U> { return b.n; }
template <class T> int A<T>::i2(C auto, const B &b) { return b.n; }
template <class T> template <class U> int A<T>::o(U, const B &b) { return b.n; }
template <class T> template <class U> int A<T>::o(U, const B &b) requires C<U> { return b.n; }
int main() {
  B b;
  return A<int>::f(1, b) + A<int>::j(1, b) + A<int>::g(1, b) + A<int>::g2(1, b) +
         A<int>::h(1, b) + A<int>::h2(1, b) + A<int>::i(1, b) + A<int>::m(1, b) +
         A<int>::q(1, b) + A<int>::k(1, b) + A<int>::r(1, b) + A<int>::t(1, b) +
         A<int>::i2(1, b) + A<int>::o(1, b) + A<int>::o('c', b);
}
