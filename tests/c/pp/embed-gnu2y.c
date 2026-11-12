// WP-1331, C23 6.10.4: no extension warning after C23 either (gnu2y).
//@ args: -std=gnu2y -Werror
//@ expect: run
//@ stdout: 3
#include <stdio.h>

static const unsigned char data[] = {
#embed "Inputs/embed-data.txt"
};

int main(void) {
  printf("%zu\n", sizeof data);
  return 0;
}
