// [basic.indet]/1 (C++26, P2795R5), [dcl.init.aggr]/4-5: an aggregate
// whose initializer writes every byte of it (no padding, every member
// given, defaulted or value-initialized) holds no erroneous value, so
// under kiln's C++26 default fill it gets no pre-fill: the IR is clang
// 22's, which has no erroneous values and fills nothing (spec/codegen.md
// §7y, "Redundant pre-fill under the default", WP-1921). A padded type,
// a union, a constructor and a self-referencing initializer keep their
// fill (crates/kiln_driver/tests/erroneous_init.rs).
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu
void use(void *);
struct Agg {
  double a = 1.0;
  double b;
};
struct Two {
  int a;
  int b;
};
struct Nest {
  Two t;
  long l;
};
struct Base {
  long x;
};
struct Derived : Base {
  long y;
};
struct BF {
  unsigned a : 16, b : 16;
};
struct WithArr {
  int v[3];
};
struct Ptrs {
  int *p;
  int Two::*m;
  _Complex double c;
};
enum E { e0, e1 };
struct WithEnum {
  E e;
  int i;
};
void s_dmi(double y) {
  Agg h{.b = y};
  use(&h);
}
void s_two(int y) {
  Two t{y, y};
  use(&t);
}
void s_two_partial(int y) {
  Two t{y};
  use(&t);
}
void s_empty_list() {
  Two t{};
  use(&t);
}
void s_nest(int y) {
  Nest n{{y, y}, y};
  use(&n);
}
void s_base(long y) {
  Derived d{{y}, y};
  use(&d);
}
void s_arr_filler(int y) {
  int arr[4] = {y};
  use(arr);
}
void s_arr_full(int y) {
  int arr[2] = {y, y};
  use(arr);
}
void s_arr_of_agg(int y) {
  Two arr[3] = {{y, y}, {y}};
  use(arr);
}
void s_bitfields(unsigned y) {
  BF b{y, y};
  use(&b);
}
void s_member_array(int y) {
  WithArr w{{y, y, y}};
  use(&w);
}
void s_paren(int y) {
  Two t(y, y);
  use(&t);
}
void s_scalars(int *p, double d) {
  Ptrs s{p, &Two::b, d};
  use(&s);
}
void s_enum(E e, int y) {
  WithEnum w{e, y};
  use(&w);
}
