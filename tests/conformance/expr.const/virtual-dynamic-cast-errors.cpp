// [expr.dynamic.cast]/9, [expr.const]/5: a failed reference dynamic_cast
// is a throw and so not a constant expression; clang's notes for each
// reason (its select index), and for a null operand.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:16 error "constexpr variable 'x1' must be initialized by a constant expression"
//@ diag: 28:52 note "reference dynamic_cast failed: dynamic type 'B' of operand does not have a base class of type 'C'"
//@ diag: 29:21 note "in call to 'no_base()'"
//@ diag: 33:16 error "constexpr variable 'x2' must be initialized by a constant expression"
//@ diag: 32:54 note "dynamic_cast of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 33:21 note "in call to 'null_ref()'"
//@ diag: 38:16 error "constexpr variable 'x3' must be initialized by a constant expression"
//@ diag: 37:46 note "reference dynamic_cast failed: static type 'A' of operand is a non-public base class of dynamic type 'P'"
//@ diag: 38:21 note "in call to 'hidden()'"
//@ diag: 46:16 error "constexpr variable 'x4' must be initialized by a constant expression"
//@ diag: 45:53 note "reference dynamic_cast failed: 'A' is an ambiguous base class of dynamic type 'LR' of operand"
//@ diag: 46:21 note "in call to 'ambiguous()'"
//@ diag: 51:16 error "constexpr variable 'x5' must be initialized by a constant expression"
//@ diag: 50:59 note "reference dynamic_cast failed: 'B' is a non-public base class of dynamic type 'Q' of operand"
//@ diag: 51:21 note "in call to 'protected_target()'"
//@ requires: clang<22
// clang 22 emits error: constexpr variable 'x2' must be initialized by a constant expression; WP-355 residual
struct A { constexpr virtual ~A() {} };
struct B : A {};
struct C : B {};

// Kind 1: the dynamic type has no base of the target type.
constexpr bool no_base() { B b; A &ra = b; C &rc = dynamic_cast<C &>(ra); return &rc == &b; }
constexpr bool x1 = no_base();

// A null pointer dereferenced to form the operand.
constexpr bool null_ref() { A *pa = nullptr; B &rb = dynamic_cast<B &>(*pa); return &rb == pa; }
constexpr bool x2 = null_ref();

// Kind 0: the static type is a non-public base of the dynamic type.
struct P : private A { constexpr const A &self() const { return *this; } };
constexpr bool hidden() { P p; const P &rp = dynamic_cast<const P &>(p.self()); return &rp == &p; }
constexpr bool x3 = hidden();

// Kind 2: the target is an ambiguous base of the dynamic type.
struct L : A {};
struct R : A {};
struct T { constexpr virtual ~T() {} };
struct LR : L, R, T {};
constexpr bool ambiguous() { LR o; T &rt = o; (void)dynamic_cast<A &>(rt); return true; }
constexpr bool x4 = ambiguous();

// Kind 3: the target is a non-public base of the dynamic type.
struct Q : T, protected B {};
constexpr bool protected_target() { Q q; T &rt = q; (void)dynamic_cast<B &>(rt); return true; }
constexpr bool x5 = protected_target();

// Pointer forms of the same failures are null, not errors.
constexpr bool pointers() {
  B b; LR o; Q q;
  return dynamic_cast<C *>(static_cast<A *>(&b)) == nullptr &&
         dynamic_cast<A *>(static_cast<T *>(&o)) == nullptr &&
         dynamic_cast<B *>(static_cast<T *>(&q)) == nullptr;
}
static_assert(pointers());
