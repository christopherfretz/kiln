// [expr.const.imm]/1.2, [expr.const.defns]/1.6, [expr.const.init]/4,
// [stmt.expand]: a constexpr local variable of a function template, or
// of an expansion statement's body, whose initializer is dependent but
// holds a non-dependent consteval call, is checked in each
// instantiation and expansion: the initializer is manifestly
// constant-evaluated, so the call is validated by the variable's check,
// not on its own, and the heap its V owns is freed at the end of the
// full-expression (spec/sema.md "Immediate invocations"; WP-1263). GCC
// 16.1 accepts this and prints the same; clang 19 and 22 check the
// call on its own in the template definition and reject it, though
// they accept the same initializer outside a template.
//@ expect: run
//@ skip(clang): clang 19 and 22 reject the non-dependent call in the template definition
//@ oracle: gcc
//@ strict-diags
//@ stdout: 3 5 7 9
extern "C" int printf(const char*, ...);
struct V {
  int* p;
  constexpr V() : p(new int(2)) {}
  constexpr ~V() { delete p; }
  constexpr int get() const { return *p; }
};
consteval V make() { return V(); }

template <class T> int f() {
  constexpr int x = make().get() + sizeof(T);
  return x;
}

template <int N> struct W {
  static int g() {
    constexpr int y = make().get() + N;
    return y;
  }
};

int main() {
  constexpr int z = make().get();
  printf("%d %d", f<char>(), W<z + 1>::g());
  template for (constexpr int k : {5, 7}) {
    constexpr int w = make().get() + k;
    printf(" %d", w);
  }
  printf("\n");
}
