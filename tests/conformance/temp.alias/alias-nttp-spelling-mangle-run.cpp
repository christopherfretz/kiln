// [temp.alias]/2, [temp.type]/1, [basic.link] (Itanium C++ ABI 5.1.5):
// an alias template specialization is transparent in a mangled name, so a
// non-type argument mangles as the aliased type's converted value however
// it is written: `Arr<char, 1 + 2>`, `Arr<char, (short)3>` and
// `EB<static_cast<E>(1), 1>` (an enumerator and a `bool`) mangle and print
// through `typeid` as `char[3]` and `int[4]`. Each function is called
// through a declaration whose asm label is clang's mangled name, so a
// different kiln mangling fails to link (spec/sema-templates.md §4.10,
// WP-1168).
//@ expect: run
//@ stdout: A3_c A3_c A4_i | PFiRA3_cE
//@ stdout: k 1 2 3
#include <cstdio>
#include <typeinfo>

template <class T, int N> using Arr = T[N];
enum class E { a, b };
template <E e, bool B> using EB = int[int(e) + 2 * B + 1];

int k(Arr<char, 1 + 2>&) { return 1; }
int k2(Arr<char, (short)3>*) { return 2; }
int k3(EB<static_cast<E>(1), 1>&) { return 3; }
int m1(char (&)[3]) __asm__("_Z1kRA3_c");
int m2(char (*)[3]) __asm__("_Z2k2PA3_c");
int m3(int (&)[4]) __asm__("_Z2k3RA4_i");

int main() {
  std::printf("%s %s %s | %s\n", typeid(Arr<char, 1 + 2>).name(),
              typeid(Arr<char, (short)3>).name(), typeid(EB<E::b, true>).name(),
              typeid(&k).name());
  char c[3] = {};
  int i[4] = {};
  std::printf("k %d %d %d\n", m1(c), m2(&c), m3(i));
}
