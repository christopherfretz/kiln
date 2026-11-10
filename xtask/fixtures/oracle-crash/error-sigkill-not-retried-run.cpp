//@ expect: run
//@ stdout: ok
// fake-clang: sigkill-once
extern "C" int printf(const char *, ...);
int main() { printf("ok\n"); }
