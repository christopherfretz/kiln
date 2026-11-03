// [dcl.fct.def.default]/3 (C++23, P2448R2): the same defaulted members
// declared consteval are accepted; only a destructor still cannot be
// consteval ([dcl.constexpr]/2) and the <=> with a non-category return type
// is still deleted ([class.spaceship]/2).
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:18 warning "explicitly defaulted three-way comparison operator is implicitly deleted"
//@ diag: 17:6 note "return type of defaulted 'operator<=>' cannot be deduced because return type 'int' of three-way comparison for member 'm' is not a standard comparison category type"
//@ diag: 14:195 note "selected 'operator<=>' for member 'm' declared here"
//@ diag: 24:48 note "replace 'default' with 'delete'"
//@ diag: 29:3 error "destructor cannot be declared consteval"
//@ diag: 27:3 error "defaulted definition of copy constructor cannot be marked consteval before C++23"
struct NC { constexpr NC() {} NC(const NC&) {} NC(NC&&) {} NC& operator=(const NC&) { return *this; } NC& operator=(NC&&) { return *this; } bool operator==(const NC&) const { return true; } int operator<=>(const NC&) const { return 0; } };
struct V {};
struct A {
  NC m;
  A() = default;
  consteval A(const A&) = default;
  consteval A(A&&) = default;
  consteval A& operator=(const A&) = default;
  consteval A& operator=(A&&) = default;
  consteval bool operator==(const A&) const = default;
  consteval auto operator<=>(const A&) const = default;
};
struct B : virtual V {
  consteval B(const B&) = default;
  consteval B& operator=(const B&) = default;
  consteval ~B() = default;
};
