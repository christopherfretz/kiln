// The escape rules of the Oven's register locals (spec/oven.md "Register
// locals"), one pair per rule: each q_* function keeps its local in a
// register (no address of it can be observed), each m_* one keeps it in
// abstract memory. Both compute the same kind of value; cases.cpp checks
// the values (with every compiler), cases-oven.cpp checks where kiln keeps
// the locals (the bytes of abstract memory each evaluation allocates).
#include <memory>
#include <utility>

// [expr.unary.op]/3: the operand of unary &. (A scalar's pseudo-destructor
// call, [expr.prim.id.dtor]/2, is in diagnostics.cpp.)
constexpr int q_addr(int n) {
  int x = n;
  x += 1;
  return x;
}
constexpr int m_addr(int n) {
  int x = n;
  int *p = &x;
  *p += 1;
  return x;
}

// [dcl.init.ref]/5: bound to a reference (a variable or a parameter).
constexpr void bump(int &r) { ++r; }
constexpr int q_ref(int n) {
  int x = n;
  int y = x;
  return x + y;
}
constexpr int m_ref(int n) {
  int x = n;
  int &r = x;
  r += 1;
  return x;
}
constexpr int m_ref_param(int n) {
  int x = n;
  bump(x);
  return x;
}

// [expr.prim.lambda.capture]/10, /12: captured by copy (the closure gets a
// copy) or by reference.
constexpr int q_capture_copy(int n) {
  int x = n;
  auto f = [x] { return x + 1; };
  return f();
}
constexpr int m_capture_ref(int n) {
  int x = n;
  auto f = [&x] { ++x; };
  f();
  return x;
}
constexpr int m_capture_default_ref(int n) {
  int x = n;
  auto f = [&] { ++x; };
  f();
  return x;
}

// [specialized.addressof]: std::addressof and __builtin_addressof take the
// address.
constexpr int m_addressof(int n) {
  int x = n;
  *std::addressof(x) += 1;
  return x;
}
constexpr int m_builtin_addressof(int n) {
  int x = n;
  *__builtin_addressof(x) += 1;
  return x;
}

// [utility.as.const], [forward]: std::move, std::forward and std::as_const
// designate their operand (clang evaluates them as no call), so a local
// moved from or forwarded keeps its register; one bound to the result
// does not.
constexpr int q_move(int a, int b) {
  int t = std::move(a);
  a = std::move(b);
  b = std::move(t);
  return a * 10 + b;
}
constexpr int q_forward(int n) {
  int x = n;
  std::forward<int &>(x) += 2;
  ++std::forward<int &>(x);
  return std::as_const(x);
}
constexpr int m_move_bound(int n) {
  int x = n;
  int &&r = std::move(x);
  r += 1;
  return x;
}

// [expr.context]/1, [dcl.type.decltype]/1: unevaluated operands do not use
// the object.
constexpr int q_unevaluated(int n) {
  int x = n;
  using R = decltype((x));
  return x + static_cast<int>(sizeof(x)) + static_cast<int>(sizeof(R));
}
constexpr int m_unevaluated_and_ref(int n) {
  int x = n;
  decltype((x)) r = x;
  r += 1;
  return x;
}

// [dcl.type.cv]/4: a const_cast glvalue that drops const may write.
constexpr int q_const_read(int n) {
  const int x = n;
  return x + const_cast<const int &>(x);
}
constexpr int m_const_cast(int n) {
  int x = n;
  const int &c = x;
  const_cast<int &>(c) += 1;
  return x;
}

// [stmt.dcl]/3, [dcl.init.general]/12: a scalar declared without an
// initializer is in a register when it is assigned before any other use,
// on every path, and no label, case or default in its scope can be jumped
// to past the declaration.
constexpr int q_assigned_first(int n) {
  int x;
  int y = n;
  x = y * 2;
  return x;
}
constexpr int q_assigned_both_branches(bool b) {
  int x;
  if (b)
    x = 1;
  else {
    x = 2;
  }
  return x;
}
constexpr int q_assigned_each_iteration(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i) {
    long x;
    if (i % 2)
      x = 1;
    else
      x = 2;
    s += x;
  }
  return s;
}
constexpr int m_assigned_one_branch(bool b) {
  int x;
  if (b)
    x = 1;
  x = b ? x : 2;
  return x;
}
constexpr int m_case_in_scope(int n) {
  int r = 0;
  switch (n) {
    int x;
  case 0:
    x = 1;
    r = x;
    break;
  default:
    r = 2;
  }
  return r;
}
constexpr int q_case_out_of_scope(int n) {
  int r = 0;
  switch (n) {
  case 0: {
    int x;
    x = 1;
    r = x;
    break;
  }
  default:
    r = 2;
  }
  return r;
}
constexpr int q_reassigned(int n) {
  int x;
  int y = n;
  x = n;
  x = x + y;
  return x;
}
constexpr int m_assigned_in_loop(int n) {
  int x;
  for (int i = 0; i < 1; ++i)
    x = n;
  return x;
}
