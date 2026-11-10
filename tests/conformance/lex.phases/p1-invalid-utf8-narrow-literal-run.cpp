// [lex.phases]/1.1, [lex.string], [lex.ccon]: Latin-1 bytes (not valid UTF-8)
// in an ordinary string or character literal are kept as they are, as clang
// and GCC do: the program sees the source bytes. A comment may hold them too:
// café à la crème.
//@ expect: run
//@ stdout: 63 61 66 e9 0 | 5 | e9 | 61e9 | c3a9e9
#include <cstdio>
int main() {
  const char *s = "café";
  for (int i = 0; i <= 4; ++i)
    std::printf("%x ", static_cast<unsigned char>(s[i]));
  std::printf("| %zu | %x", sizeof("café"), static_cast<unsigned char>('é'));
  std::printf(" | %x | %x\n", 'aé', 'Ã©é');
}
