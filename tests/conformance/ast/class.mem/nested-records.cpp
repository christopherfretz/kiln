//@ expect: ast
struct Inner { short a; char b; };
struct Outer {
  Inner in;
  Inner *pin;
  int arr[2];
};
int sum(Outer o, Outer *po) {
  return o.in.a + o.pin->b + po->in.a + po->arr[1] + (*po).arr[0];
}
