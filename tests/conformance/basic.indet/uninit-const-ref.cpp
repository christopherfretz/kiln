// [basic.indet], [dcl.init.ref]: -Wuninitialized-const-reference warns
// when a local that is uninitialized on every path is passed to a const
// reference parameter, once per variable. (clang 21's const-pointer form,
// -Wuninitialized-const-pointer, is not implemented.)
//@ expect: compile-ok
//@ args: -Wuninitialized -Wuninitialized-const-reference
//@ strict-diags
//@ diag: 20:8 warning "variable 'w' is uninitialized when passed as a const reference argument here"
//@ diag: 25:8 warning "variable 'w' is uninitialized when passed as a const reference argument here"
void take(const int &);
void take_two(const int &, int);
struct R {
  int a;
  R() = default;
};
void take_rec(const R &);

void plain() {
  int w;
  take(w);
}

void second_use() {
  int w;
  take(w);
  take(w);
}

void after_init() {
  int w;
  w = 1;
  take(w);
}

void record() {
  R r;
  take_rec(r);
}

void conditional(bool b) {
  int w;
  if (b)
    w = 1;
  take(w);
}
