//@ expect: run
//@ stdout: ok
// fake-clang: oom-once
extern "C" int printf(const char *, ...);
int main() { printf("ok\n"); }
