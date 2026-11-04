// clang's `enable_if` attribute (clang AttrDocs "enable_if", `Sema::IsOverload`;
// [class.mem.general]/5, [class.mfct]/3): member functions that differ only
// in their `enable_if` conditions are overloads, in the class and when
// defined out of line (each out-of-line definition matches the declaration
// with its conditions); a static member function too.
//@ expect: run
//@ strict-diags
struct S {
  int get(int n) const { return n; }
  int get(int n) const __attribute__((enable_if(n == 0, "zero"))) { return 50; }
  int put(int n);
  int put(int n) __attribute__((enable_if(n == 1, "one")));
  static int st(int n) { return n + 1; }
  static int st(int n) __attribute__((enable_if(n == 2, "two")));
};
int S::put(int n) __attribute__((enable_if(n == 1, "one"))) { return 10; }
int S::put(int n) { return n + 20; }
int S::st(int n) __attribute__((enable_if(n == 2, "two"))) { return 30; }
int main() {
  S s;
  if (s.get(0) != 50 || s.get(3) != 3) return 1;
  if (s.put(1) != 10 || s.put(2) != 22) return 2;
  if (S::st(2) != 30 || S::st(5) != 6) return 3;
  return 0;
}
