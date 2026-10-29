// [basic.start.main]: flowing off the end of main returns 0.
//@ expect: run
//@ stdout: no return statement
extern "C" int printf(const char*, ...);

int main() { printf("no return statement\n"); }
