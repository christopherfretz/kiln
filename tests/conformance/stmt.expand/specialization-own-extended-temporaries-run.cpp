// [temp.inst], [stmt.expand]/5.3, [class.temporary]/8-9 (CWG3043): two
// specializations of a function template whose expansion statements have
// non-dependent initializers each extend temporaries of their own: the
// destructuring form's range temporary and the expansion-init-list's
// element temporaries are mutated through the element and destroyed
// after the statement (the range) or the element (an element temporary),
// once per specialization. GCC 16.1 agrees; clang 22 has no P1306.
// WP-3291; spec/sema-templates.md.
//@ expect: run
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ stdout: 11 12 ~D12 ~D11 | 13 ~D13 14 ~D14 ; 21 22 ~D22 ~D21 | 23 ~D23 24 ~D24 ; end
extern "C" int printf(const char *, ...);
struct D {
  int v;
  ~D() { printf("~D%d ", v); }
};
struct P {
  D x, y;
};
template <class T> void run(int k) {
  template for (auto &&d : P{D{1}, D{2}}) {
    d.v += k;
    printf("%d ", d.v);
  }
  printf("| ");
  template for (D &&d : {D{3}, D{4}}) {
    d.v += k;
    printf("%d ", d.v);
  }
  printf("; ");
}
int main() {
  run<int>(10);
  run<long>(20);
  printf("end\n");
}
