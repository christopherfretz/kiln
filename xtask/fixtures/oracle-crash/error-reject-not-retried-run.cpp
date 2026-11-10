//@ expect: run
//@ stdout: ok
// fake-clang: reject-once
extern "C" int printf(const char *, ...);
int main() { printf("ok\n"); }
