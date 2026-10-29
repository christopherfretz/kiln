// [lex.ccon] + [lex.string]: simple, octal and hexadecimal escape sequences
// in character and string literals.
//@ expect: run
//@ stdout: 65 66 67 9 0 92 39
//@ stdout: A"B'C
extern "C" int printf(const char*, ...);

int main() {
  char c1 = 'A', c2 = '\x42', c3 = '\103', tab = '\t', nul = '\0', bs = '\\', q = '\'';
  printf("%d %d %d %d %d %d %d\n", c1, c2, c3, tab, nul, bs, q);
  printf("\x41\"\102\'C\n");
}
