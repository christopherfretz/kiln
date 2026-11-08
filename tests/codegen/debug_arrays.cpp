// Debug info (WP-814): a global array, an array reference parameter and a pointer parameter.
//@ args: -g
int arr[4] = {1, 2, 3, 4};
int f(int (&a)[3], const int *p) {
  int loc[2] = {p[0], a[1]};
  return loc[0] + loc[1] + arr[2];
}
