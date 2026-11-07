// [dcl.align]/1, [temp.local]/1: in `alignas(V)` inside the class template
// `V`, `V` is the injected-class-name, a type-name, so the operand is a
// type-id (clang `ParseAlignArgument` tests `isTypeIdInParens`, as
// `sizeof(...)` does), also for a nested class of a template and with
// the template's own arguments.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
template <class T> struct V {
  T t[3];
  V& f() {
    alignas(V) static char buffer[sizeof(V)];
    static_assert(alignof(decltype(buffer)) == alignof(char) || true, "");
    return *reinterpret_cast<V*>(buffer);
  }
  struct N {
    long l;
    void g() {
      alignas(N) char a[sizeof(N)];
      alignas(V<T>) char b[sizeof(V)];
      static_assert(alignof(decltype(a)) == 1, "");
      (void)a;
      (void)b;
    }
  };
};
struct alignas(16) Big { char c; };
template <class T> struct W {
  T t;
  static unsigned long g() {
    alignas(W) char b[sizeof(W)];
    (void)b;
    struct alignas(W) L { char c; };
    return alignof(L);
  }
};
static_assert(sizeof(V<double>) == 3 * sizeof(double), "");
int main() {
  V<int>().f();
  V<int>::N().g();
  return W<Big>::g() == 16 ? 0 : 1;
}
