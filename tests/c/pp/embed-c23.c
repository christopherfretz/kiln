// WP-1331, C23 6.10.4: #embed is standard C23, so clang 19/22 give no
// extension warning and a -Werror build compiles; only the off-by-default
// -Wpre-c23-compat names it.
//@ std: c23
//@ args: -Werror
//@ expect: run
//@ stdout: 3 105 116
#include <stdio.h>

static const unsigned char data[] = {
#embed "Inputs/embed-data.txt"
};

int main(void) {
  printf("%zu %d %d\n", sizeof data, data[0], data[2]);
  return 0;
}
