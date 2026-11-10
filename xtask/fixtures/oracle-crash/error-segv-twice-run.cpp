//@ expect: run
//@ stdout: ok
// fake-clang: segv-always
extern "C" int printf(const char *, ...);
int main() { printf("ok\n"); }
