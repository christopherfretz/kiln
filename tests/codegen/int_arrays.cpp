// Local int arrays: sizes below and above 16 bytes, zero, full and partial
// initializers.
int use(int*);
int arrays(int x) {
  int a2[2] = {x, x};
  int a3[3] = {1, 2, 3};
  int a4[4] = {};
  int a5[5] = {x};
  int a8[8] = {1, 2, 3, 4, 5, 6, 7, 8};
  int a16[16];
  a16[3] = x;
  long l3[3] = {1, 2};
  return use(a2) + use(a3) + use(a4) + use(a5) + use(a8) + use(a16) + (int)l3[1];
}
