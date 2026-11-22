//@ expect: run
//@ oracle: none
int main() {
  volatile int x = 0;
  while (x == 0) {
  }
}
