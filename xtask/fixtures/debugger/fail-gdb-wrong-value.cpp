// A check that no output line matches fails the test.
//@ expect: debugger
//@ gdb: break fail-gdb-wrong-value.cpp:9
//@ gdb: run
//@ gdb: print k
//@ gdb-check: $1 = 43
int main() {
  int k = 42;
  return k - 42;
}
