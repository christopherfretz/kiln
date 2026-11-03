// Clang ext vectors, [clang 20+]: a bool ext vector condition picks each lane
// from the second operand where the lane is true, else from the third. WP-187.
//@ expect: run
//@ requires: clang>=22
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef int v4i __attribute__((ext_vector_type(4)));
v4i sel(b4 m, v4i x, v4i y) { return m ? x : y; }
b4 mk(v4i x, v4i y) { return __builtin_convertvector(x < y, b4); }
int main() {
  v4i x = {1, 2, 3, 4}, y = {5, 0, 7, 0};
  v4i r = sel(mk(x, y), x, y);
  return (r[0] == 1 && r[1] == 0 && r[2] == 3 && r[3] == 0) ? 0 : 1;
}
