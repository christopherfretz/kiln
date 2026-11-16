// [class.friend]/4, [temp.friend]/1, [module.reach]/2: the dependent
// special member friends of a module's class template are matched in each
// specialization an importer instantiates (WP-1507). Under `--bmi-image` the
// module falls back to replaying its source ("a friend specialization or
// member of a class template").
//@ skip(clang): clang befriends no constructor or destructor of a dependent class
//@ expect: run
//@ std: c++20
//@ module: dsf=Inputs/dependent-special-friend.cppm
//@ stdout: 1 2 3
import dsf;
extern "C" int printf(const char *, ...);
struct P { typedef int t; };
int main() {
  Z<P> z(0);
  printf("%d ", int(z));
}
