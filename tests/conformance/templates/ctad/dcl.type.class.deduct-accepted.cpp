// [dcl.type.class.deduct]/2: the contexts that do admit a deduced class
// type placeholder.
//@ expect: run
//@ strict-diags
template<class T> struct B { constexpr B(T v) : t(v) {} T t; };
B b1{1};
B b2(1);
B b3 = 1;
auto c1 = B(1);
auto c2 = B{1};
auto c3 = static_cast<B<int>>(1);
const B cb(1);
static B sb(1);
struct S { static inline B si{1}; };
int arr[3];
void fr() {
  for (B y : arr) {}
  if (B z = 1; z.t) {}
}
auto sz = sizeof(B(1));
int main() {
  auto *n1 = new B(1);
  auto *n2 = new B{1};
  int r = b1.t + b2.t + n1->t + n2->t + c2.t - 5;
  delete n1;
  delete n2;
  return r;
}
