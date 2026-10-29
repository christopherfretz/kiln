// [dcl.init.string]: a char array initialized by a string literal gets the
// characters and the terminating null; extra elements are zero.
//@ expect: run
//@ stdout: hello 6 | hi 0 0 | abc
extern "C" int printf(const char*, ...);

int main() {
  char s[] = "hello";
  char t[5] = "hi";
  char u[4] = {'a', 'b', 'c'};
  printf("%s %zu | %s %d %d | %s\n", s, sizeof s, t, t[3], t[4], u);
}
