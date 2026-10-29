// [stmt.switch]: control jumps to the matching case label (or default) and
// falls through subsequent labels until a break.
//@ expect: run
//@ stdout: one two three | two three | default four | five
extern "C" int printf(const char*, ...);

void show(int v) {
  switch (v) {
    case 1:
      printf("one ");
      [[fallthrough]];
    case 2:
      printf("two ");
    case 3:
      printf("three ");
      break;
    default:
      printf("default ");
    case 4:
      printf("four ");
      break;
    case 5:
      printf("five");
  }
}

int main() {
  show(1);
  printf("| ");
  show(2);
  printf("| ");
  show(9);
  printf("| ");
  show(5);
  printf("\n");
}
