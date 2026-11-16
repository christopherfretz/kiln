// [temp.friend]/1, /5: the friend constructor template and conversion
// function template of a dependent class, and the friend constructor of
// every specialization of a class template, from a precompiled header are
// matched in each specialization the consumer instantiates (WP-1513).
//@ skip(clang): clang befriends no constructor of a dependent class
//@ expect: run
//@ pch: Inputs/special-member-template-friend.h
//@ stdout: 1 2 3
int main() {
  Z<int> z(0);
  (void)(char *)z;
  X<int> x(0);
}
