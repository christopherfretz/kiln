// clang's `enable_if` attribute (clang AttrDocs "enable_if", `Sema::IsOverload`;
// [class.mem.general]/5, [over.match.best] as clang extends it): two
// constructors that differ only in their `enable_if` conditions are
// overloads, not a redeclaration. `Lock(B)` selects the constrained one
// (its conditions extend the other's empty list) and delegates; `Lock(A)`
// finds it not viable. The `[[deprecated]]` one warns where it is selected.
// A spin-lock constructor shape from a widely used C++ library; kiln said
// "constructor cannot be redeclared" until WP-579.
//@ expect: run
//@ strict-diags
//@ diag: 23:30 warning "'Lock' is deprecated: x"
//@ diag: 17:5 note "'Lock' has been explicitly marked deprecated here"
enum Mode { A, B };
struct Lock {
  constexpr Lock() : w_(0) {}
  constexpr explicit Lock(Mode m) : w_(m == A ? 1 : 0) {}
  [[deprecated("x")]]
  constexpr explicit Lock(Mode m)
      __attribute__((enable_if(m == B, "use default constructor")))
      : Lock() {}
  int w_;
};
int main() { Lock a(A); Lock b(B); return a.w_ + b.w_ - 1; }
