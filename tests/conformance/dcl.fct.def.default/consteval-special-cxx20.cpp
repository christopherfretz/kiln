// [dcl.fct.def.default]/3, [dcl.constexpr]/3 (C++20): an explicitly
// defaulted special member declared consteval whose implicit definition
// would not be constexpr is an error before C++23 (copy/move constructors
// and assignments calling non-constexpr ones; a class with a virtual base);
// a defaulted comparison names the non-constexpr function it would call. A
// destructor is never consteval ([dcl.constexpr]/2).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:3 error "defaulted definition of copy constructor cannot be marked consteval before C++23"
//@ diag: 30:3 error "defaulted definition of move constructor cannot be marked consteval before C++23"
//@ diag: 31:3 error "defaulted definition of copy assignment operator cannot be marked consteval before C++23"
//@ diag: 32:3 error "defaulted definition of move assignment operator cannot be marked consteval before C++23"
//@ diag: 33:3 error "defaulted definition of equality comparison operator cannot be declared consteval because it invokes a non-constexpr comparison function"
//@ diag: 27:6 note "non-constexpr comparison function would be used to compare member 'm'"
//@ diag: 24:146 note "non-constexpr comparison function declared here"
//@ diag: 34:18 warning "explicitly defaulted three-way comparison operator is implicitly deleted"
//@ diag: 27:6 note "return type of defaulted 'operator<=>' cannot be deduced because return type 'int' of three-way comparison for member 'm' is not a standard comparison category type"
//@ diag: 24:195 note "selected 'operator<=>' for member 'm' declared here"
//@ diag: 34:48 note "replace 'default' with 'delete'"
//@ diag: 39:3 error "destructor cannot be declared consteval"
//@ diag: 37:3 error "defaulted definition of copy constructor cannot be marked consteval before C++23"
//@ diag: 38:3 error "defaulted definition of copy assignment operator cannot be marked consteval before C++23"
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
