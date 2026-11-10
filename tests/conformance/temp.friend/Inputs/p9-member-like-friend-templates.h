// Shared by ../p9-member-like-friend-template-mangling-two-units-run.cpp
// and its -19 sibling: friend templates with constraints ([temp.friend]/9).
template <class T> concept C = sizeof(T) > 1;
// A friend template defined in a class with a trailing requires-clause:
// its specializations are member-like (a nested name, no class prefix).
struct S {
  template <class U> friend int n1(U, S *) requires C<U> { return 1; }
  // Defined at namespace scope: not member-like.
  template <class U> friend int n2(U, S *) requires C<U>;
};
template <class U> int n2(U, S *) requires C<U> { return 2; }
// Declared at namespace scope first, defined as a friend: member-like
// for clang 19, not for clang 22.
template <class U> int n3(U) requires C<U>;
struct S3 {
  template <class U> friend int n3(U) requires C<U> { return 3; }
};
template <class T> struct A {
  // The trailing requires-clause names only the friend's own parameter.
  template <class U> friend int g1(U, A *) requires C<U> { return 10; }
  // It names the enclosing class's `T`: `F`, and from clang 20 on the
  // class's prefix.
  template <class U> friend int g2(U, A *) requires C<T> { return 20; }
  // A template-head requires-clause naming `T`: `F`, unscoped.
  template <class U> requires C<T> friend int g3(U, A *) { return 30; }
};
