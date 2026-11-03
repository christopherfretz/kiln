// [temp.friend]/1, [temp.over.link]/6: a constrained friend function
// template declared in a class template redeclares the namespace scope
// template with an equivalent head once the class is instantiated; its
// type-constraints and requires-clauses are compared after substituting
// the specialization's arguments, its own parameters one level shallower.
// libc++ 22's stack/queue declare `template<class T1, three_way_comparable
// C> friend operator<=>` and define it at namespace scope (WP-446 cluster
// "constrained friend template redeclaration").
//@ expect: run
//@ std: c++26
template <class T> concept C = sizeof(T) > 0;

template <class T, class U> struct S;
template <class T, C U> int f(const S<T, U> &);
template <class T, class U> int g(const S<T, U> &) requires C<U>;

template <class T, class U> struct S {
  template <class T1, C U1> friend int f(const S<T1, U1> &);
  template <class T1, class U1> friend int g(const S<T1, U1> &) requires C<U1>;
  template <class T1, C U1> friend int h(const S<T1, U1> &);
};

template <class T, C U> int f(const S<T, U> &) { return 1; }
template <class T, class U> int g(const S<T, U> &) requires C<U> { return 2; }
template <class T, C U> int h(const S<T, U> &) { return 3; }

int main() {
  S<int, long> s;
  return f(s) + g(s) + h(s) == 6 ? 0 : 1;
}
