// [temp.friend]/5, [temp.over.link]/6-7: a friend member function template
// of each A<T> declared in a member class template O<X>::B<Y> names, of an
// overload set differing only by a trailing requires-clause, the member
// with the equivalent one. In the pattern of B the friend's outer levels
// are not known, so it only might befriend a member (decided per
// instantiation, WP-1146); O<int>::B<char> befriends A<int>::o(U, const V &)
// requires C<U> and not the unconstrained o, whose requires-expression
// then sees b.n as inaccessible. clang does not model these friends
// (-Wunsupported-friend); GCC 16.1 rejects the friend ("no declaration
// matches").
//@ skip(clang): clang does not model [temp.friend]/5 friends (-Wunsupported-friend)
//@ expect: run
//@ strict-diags
//@ stdout: 200 1
//@ diag: 23:69 warning "for friend class declaration is not supported; turning off access control for 'B'"
template <class T> concept C = sizeof(T) > 1;
template <class T> struct A {
  template <class U, class V> static int o(U, const V &);
  template <class U, class V> static int o(U, const V &) requires C<U>;
};
template <class X> struct O {
  template <class Y> class B {
    template <class T> template <class U, class V> friend int A<T>::o(U, const V &) requires C<U>;
    int n = 1;
  };
};
template <class T> template <class U, class V>
int A<T>::o(U, const V &b) { return requires { b.n; } ? 100 : 200; }
template <class T> template <class U, class V>
int A<T>::o(U, const V &b) requires C<U> { return b.n; }
extern "C" int printf(const char *, ...);
int main() {
  O<int>::B<char> b;
  printf("%d %d\n", A<int>::o('c', b), A<int>::o(1, b));
}
