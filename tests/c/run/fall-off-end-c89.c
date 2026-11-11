// C89 implicit-int main and a function without a prototype that flows
// off its end: no trap, and main returns 0.
//@ std: c89
//@ expect: run
//@ args: -O2 -w
//@ stdout: done
int printf();
note(x) { if (x) return x; }
main() {
  note(0);
  printf("done\n");
}
