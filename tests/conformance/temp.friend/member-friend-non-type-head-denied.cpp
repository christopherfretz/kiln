// [temp.friend]/5, [class.access]/1: a friend `template <int N> friend int
// A<N>::impl();` of class template `B` befriends `A<N>::impl` only;
// another member of `A<N>` is still denied `B<0>`'s private member.
// The twin of member-friend-non-type-head-run.cpp.
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang does not model [temp.friend]/5 friends (it denies `impl` too)
//@ strict-diags
//@ diag: 15:37 warning "for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 23:48 error "'i' is a private member of 'B<0>'"
//@ diag: 14:7 note "implicitly declared private here"
template <int I> struct A;
template <int I> class B {
  int i = I + 1;
  template <int N> friend int A<N>::impl();
};
B<0> b1;
template <int I> struct A {
  int impl();
  int other();
};
template <int I> int A<I>::impl() { return b1.i; }
template <int I> int A<I>::other() { return b1.i; }
int main() { return A<0>().other() + A<0>().impl(); }
