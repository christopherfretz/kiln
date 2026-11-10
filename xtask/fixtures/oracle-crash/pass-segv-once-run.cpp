//@ expect: run
//@ stdout: ok
// fake-clang: segv-once
extern "C" int printf(const char *, ...);
int main() { printf("ok\n"); }
