// [expr.typeid], [dcl.fct]/6; Itanium C++ ABI §2.9.5, §5.1.5.3: the
// `type_info` of a pointer to member function whose function type has
// cv-qualifiers or a ref-qualifier (an abominable pointee) is that of
// its own type, named with the qualifiers (`M1SKFvvE`), and differs from
// the unqualified one's.
//@ expect: run
//@ stdout: M1SKFvvE M1SFvvOE M1SVKFiiRE 0 1
#include <cstdio>
#include <typeinfo>
struct S {};
int main() {
  std::printf("%s %s %s ", typeid(void (S::*)() const).name(),
              typeid(void (S::*)() &&).name(),
              typeid(int (S::*)(int) const volatile &).name());
  bool same = typeid(void (S::*)() const) == typeid(void (S::*)());
  std::printf("%d ", same);
  bool self = typeid(void (S::*)() &&) == typeid(void (S::*)() &&);
  std::printf("%d\n", self);
}
