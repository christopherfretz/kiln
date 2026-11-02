// [stmt.expr], clang `isUnusedResultAWarning` (WP-158 item j): a
// functional or C-style cast performing a constructor conversion is
// checked through its constructor call, so `X(1);` does not warn (unless
// the class or constructor is [[nodiscard]]); a braced or scalar
// functional cast does.
//@ strict-diags
//@ diag: 40:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute [-Wunused-value]"
//@ diag: 41:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute [-Wunused-value]"
//@ diag: 33:3 warning "expression result unused [-Wunused-value]"
//@ diag: 38:3 warning "expression result unused [-Wunused-value]"
//@ diag: 39:9 warning "expression result unused [-Wunused-value]"
//@ diag: 48:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute: why [-Wunused-value]"
//@ diag: 50:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute: because [-Wunused-value]"
//@ diag: 51:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute: because [-Wunused-value]"
//@ diag: 52:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute [-Wunused-value]"
//@ diag: 53:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute [-Wunused-value]"
//@ diag: 54:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute: why [-Wunused-value]"
//@ diag: 55:3 warning "ignoring temporary created by a constructor declared with 'nodiscard' attribute: why [-Wunused-value]"
//@ diag: 56:3 warning "expression result unused [-Wunused-value]"
//@ requires: clang<22
// clang 22 emits warning: ignoring temporary of type 'N' declared with 'nodiscard' attribute; WP-355 residual
struct X { int a; X(int); };
struct Y { int a; };
struct Z { Z(); Z(int); ~Z(); };
struct [[nodiscard]] N { N(int); };
struct W { [[nodiscard]] W(int); };
template <class T> void t() { T(1); (T)1; }
void j() {
  X(1);
  (X)1;
  (void)X(2);
  X{3};
  Y{4};
  Y();
  Z();
  Z(1);
  Z{};
  int(5);
  X(1), 2;
  N(6);
  W(7);
  t<X>();
}
struct [[nodiscard("why")]] N2 { N2(int); };
struct W2 { [[nodiscard("because")]] W2(int); W2(); };
struct [[nodiscard]] Q { Q(int); ~Q(); };
void k() {
  N2{6};
  W2{};
  W2(1);
  W2{1};
  Q(2);
  Q{3};
  (N2)4;
  static_cast<N2>(5);
  static_cast<X>(5);
}
