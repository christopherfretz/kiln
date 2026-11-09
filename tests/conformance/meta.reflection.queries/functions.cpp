// [meta.reflection.queries] P2996R13, [class.mem], [dcl.fct.def],
// [class.virtual], [special]: the function predicates (is_virtual,
// is_pure_virtual, is_override, is_final, is_deleted, is_defaulted,
// is_user_provided, is_user_declared, is_explicit, is_noexcept, is_const,
// is_volatile, is_lvalue_reference_qualified,
// is_rvalue_reference_qualified, is_conversion_function,
// is_operator_function, is_literal_operator) and the special-member set
// (is_special_member_function, is_constructor, is_default_constructor,
// is_copy_constructor, is_move_constructor, is_assignment,
// is_copy_assignment, is_move_assignment, is_destructor). A function
// defaulted on its first declaration is user-declared but not
// user-provided ([dcl.fct.def.default]/5); one defaulted later is
// user-provided. is_noexcept, is_const and the ref-qualifier predicates
// also take a function type. Operator and conversion functions are
// reached through members_of until ^^operator== parses. Answers
// checked against GCC 16.1 (-freflection).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include "kiln_meta_queries.h"
using namespace std::meta;

struct B {
  virtual void vf();
  virtual void pv() = 0;
  virtual void fin();
  void nv();
};
struct D final : B {
  void vf() override;
  void pv() override;
  void fin() final;
};
struct E : B {
  void vf();  // overrides without the virt-specifier
};
static_assert(is_virtual(^^B::vf) && is_virtual(^^B::pv) && !is_virtual(^^B::nv));
static_assert(is_pure_virtual(^^B::pv) && !is_pure_virtual(^^B::vf));
static_assert(is_override(^^D::vf) && is_override(^^E::vf) && !is_override(^^B::vf));
static_assert(is_virtual(^^E::vf));  // [class.virtual]/2: an overrider is virtual
static_assert(is_final(^^D) && is_final(^^D::fin) && !is_final(^^B) && !is_final(^^B::fin));

struct S {
  S() = default;                        // 0
  S(const S&);                          // 1
  S(S&&) noexcept;                      // 2
  explicit S(int);                      // 3
  S(char) = delete;                     // 4
  S& operator=(const S&) = default;     // 5
  S& operator=(S&&);                    // 6
  S& operator=(int);                    // 7
  ~S();                                 // 8
  explicit operator bool() const;       // 9
  operator int();                       // 10
  bool operator==(const S&) const;      // 11
  void cf() const;                      // 12
  void vf() volatile;                   // 13
  void lf() &;                          // 14
  void rf() &&;                         // 15
  void nx() noexcept;                   // 16
  static void sf();                     // 17
};
S::S(S&&) noexcept = default;
S& S::operator=(S&&) = default;

consteval info m(unsigned long i) { return members_of(^^S, access_context::unchecked())[i]; }

static_assert(is_constructor(m(0)) && is_default_constructor(m(0)) && is_special_member_function(m(0)));
static_assert(is_defaulted(m(0)) && is_user_declared(m(0)) && !is_user_provided(m(0)));
static_assert(is_copy_constructor(m(1)) && !is_move_constructor(m(1)) && is_user_provided(m(1)));
static_assert(is_move_constructor(m(2)) && is_defaulted(m(2)) && is_user_provided(m(2)));
static_assert(is_noexcept(m(2)) && !is_noexcept(m(1)));
static_assert(is_constructor(m(3)) && is_explicit(m(3)) && !is_special_member_function(m(3)));
static_assert(!is_explicit(m(1)) && !is_default_constructor(m(3)));
static_assert(is_deleted(m(4)) && !is_user_provided(m(4)) && is_user_declared(m(4)));
static_assert(is_assignment(m(5)) && is_copy_assignment(m(5)) && is_defaulted(m(5)));
static_assert(is_operator_function(m(5)) && is_special_member_function(m(5)));
static_assert(is_move_assignment(m(6)) && !is_copy_assignment(m(6)));
static_assert(is_assignment(m(7)) && !is_copy_assignment(m(7)) && !is_move_assignment(m(7)));
static_assert(!is_special_member_function(m(7)));
static_assert(is_destructor(m(8)) && is_special_member_function(m(8)) && !is_constructor(m(8)));
static_assert(is_noexcept(m(8)));  // [except.spec]/8: implicitly noexcept
static_assert(is_conversion_function(m(9)) && is_explicit(m(9)) && is_const(m(9)));
static_assert(!is_operator_function(m(9)));
static_assert(is_conversion_function(m(10)) && !is_explicit(m(10)) && !is_const(m(10)));
static_assert(is_operator_function(m(11)) && !is_conversion_function(m(11)) && is_const(m(11)));
static_assert(is_const(m(12)) && !is_volatile(m(12)));
static_assert(is_volatile(m(13)) && !is_const(m(13)));
static_assert(is_lvalue_reference_qualified(m(14)) && !is_rvalue_reference_qualified(m(14)));
static_assert(is_rvalue_reference_qualified(m(15)) && !is_lvalue_reference_qualified(m(15)));
static_assert(is_noexcept(m(16)) && !is_noexcept(m(17)));
static_assert(!is_virtual(m(17)) && !is_special_member_function(m(17)));

// The same qualifiers on a function type.
static_assert(is_const(^^void() const) && is_volatile(^^void() volatile));
static_assert(is_lvalue_reference_qualified(^^void() &));
static_assert(is_rvalue_reference_qualified(^^void() &&));
static_assert(is_noexcept(^^void() noexcept) && !is_noexcept(^^void()));
static_assert(!is_const(^^void()) && !is_const(^^int) && is_const(^^const int));

void free_fn();
void deleted_fn() = delete;
struct T { bool operator<(const T&) const; };
bool operator==(T, T);
static_assert(is_user_provided(^^free_fn) && is_user_declared(^^free_fn));
static_assert(!is_defaulted(^^free_fn) && !is_deleted(^^free_fn));
static_assert(is_deleted(^^deleted_fn) && !is_user_provided(^^deleted_fn));
static_assert(is_operator_function(members_of(^^T, access_context::unchecked())[0]));
static_assert(!is_operator_function(^^free_fn) && !is_literal_operator(^^free_fn));
// [expr.reflect]/7: `^^` of a literal-operator-id reflects the literal
// operator (WP-830 pins it).
int operator""_x(unsigned long long);
static_assert(is_literal_operator(^^operator""_x) && is_function(^^operator""_x));
static_assert(!is_operator_function(^^operator""_x));

// None of these holds for a non-function.
int var;
static_assert(!is_virtual(^^var) && !is_deleted(^^var) && !is_defaulted(^^var));
static_assert(!is_user_provided(^^var) && !is_explicit(^^var) && !is_noexcept(^^var));
static_assert(!is_constructor(^^S) && !is_special_member_function(^^S) && !is_final(^^int));
