// [temp.friend]/1 and /5, [temp.over.link]/6-7, [module.reach]/2: the
// member friends of a module's class templates and classes keep their
// constraints in an importer. W<int>, first instantiated in the importer,
// grants A<int>::f, A<int>::j and the constrained A<int>::o, not the
// unconstrained o; B grants A<int>::b. Under `--bmi-image` the module
// falls back to replaying its source ("a friend declaration",
// spec/sema-templates.md §4.8).
//@ skip(clang): clang does not model [temp.friend]/5 friends (-Wunsupported-friend)
//@ expect: run
//@ std: c++20
//@ module: mfr=Inputs/member-friend.cppm
//@ stdout: 3 4 5 0 5
import mfr;
extern "C" int printf(const char *, ...);
int main() {
  W<int> w;
  B b;
  printf("%d %d %d %d %d\n", A<int>::f(1, w), A<int>::j(1, w), A<int>::o(1, w),
         A<int>::o('c', w), A<int>::b(1, b));
}
