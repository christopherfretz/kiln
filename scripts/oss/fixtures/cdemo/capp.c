#include <stdio.h>
#include <stdlib.h>
#include "sum.h"

int main(void) {
  struct point pts[] = {[1] = {.y = 3, .x = 2}, [0] = {4, 5}};
  int *buf = malloc(sizeof *buf); /* void * converts implicitly in C */
  if (!buf)
    return 1;
  *buf = sum_points(pts, (int)(sizeof pts / sizeof pts[0]));
  printf("c %d\n", *buf);
  free(buf);
  return 0;
}
