// C23 6.5.17.1p3: an assignment's result is the left operand's
// new value, not an lvalue; it can initialize an object of the same
// structure type, and a member of it can be read.
//@ expect: run
struct S {
  int a, b;
};
static struct S y, z = {3, 4};
static const struct S cs = {1, 2};
int main(void) {
  struct S x = (y = z);
  struct S t;
  if (x.a != 3 || x.b != 4)
    return 1;
  if (y.b != 4)
    return 2;
  if ((t = cs).b != 2)
    return 3;
  return 0;
}
