// [temp.friend]/1, /5, [module.reach]/2: the friend constructor template and
// conversion function template of a dependent class, and the friend
// constructor of every specialization of a class template, of a module's
// class template are matched in each specialization an importer
// instantiates (WP-1513).
//@ skip(clang): clang befriends no constructor of a dependent class
//@ expect: run
//@ std: c++20
//@ module: smtf=Inputs/special-member-template-friend.cppm
//@ stdout: 1 2 3
import smtf;
int main() {
  Z<int> z(0);
  (void)(char *)z;
  X<int> x(0);
}
