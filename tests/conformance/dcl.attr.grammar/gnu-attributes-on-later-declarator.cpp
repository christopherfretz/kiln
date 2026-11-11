// [dcl.decl.general]/1, [class.mem.general]/1 (GNU extension): a later
// init-declarator or member-declarator of a group may begin with GNU
// attributes, which apply to that declarator alone (clang's
// `ParseDeclGroup` and `ParseCXXClassMemberDeclaration`; gcc.c-torture
// compile/20010714-1.c).
//@ expect: run
//@ strict-diags
//@ stdout: 1 16 16 4
#include <cstddef>
#include <cstdio>

void d0(void), __attribute__((noreturn)) d1(void), d2(void);
int a, __attribute__((aligned(16))) b, c;

struct S {
  char x, __attribute__((aligned(16))) y;
  int z;
};

void d1(void) { throw 0; }

int main() {
  std::printf("%d %d %d %d\n", (int)__alignof__(a) <= 8,
              (int)__alignof__(b), (int)offsetof(S, y),
              (int)__alignof__(c));
}
