// [class.friend]/4, [temp.friend]/1: the dependent special member friends of
// a class template from a precompiled header are matched in each
// specialization the consumer instantiates (WP-1507; the friends' side
// table is part of the PCH snapshot).
//@ skip(clang): clang befriends no constructor or destructor of a dependent class
//@ expect: run
//@ pch: Inputs/dependent-special-friend.h
//@ stdout: 1 2 3
struct P { typedef int t; };
int main() {
  Z<P> z(0);
  printf("%d ", int(z));
}
