// Local char arrays of several sizes; 16 bytes and more are align 16 on
// x86_64 (clang's large-array alignment).
int use(const char*);
int arrays() {
  char a1[1] = {0};
  char a7[7] = "abcdef";
  char a15[15] = "fifteen chars!";
  char a16[16] = "sixteen chars!!";
  char a17[17] = "seventeen chars!";
  char a32[32] = "thirty-two";
  char a100[100];
  a100[0] = 'x';
  char s[] = "deduced size";
  return use(a1) + use(a7) + use(a15) + use(a16) + use(a17) + use(a32) + use(a100) + use(s);
}
