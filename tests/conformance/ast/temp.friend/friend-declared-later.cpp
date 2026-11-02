//@ expect: ast
//@ requires: clang>=22
// [namespace.memdef]/3, [temp.friend]/1: a class or class template first
// declared by a friend declaration is hidden from ordinary lookup until a
// declaration at namespace scope, which redeclares it (WP-121; kiln once
// kept such a redeclaration hidden, as in libc++'s `basic_string`, which
// `__wrap_iter` befriends before <__fwd/string.h> declares it).
class W {
  friend class X;
};
class X;
X *p;

template <class I> class Wrap {
  template <class C, class T, class A> friend class Str;
  I i;
};
template <class C, class T = int, class A = char> class Str;
using S = Str<char>;
Str<char> *q;
