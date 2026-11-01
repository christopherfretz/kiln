// [class.mem.general]/7: a trailing requires-clause is not a complete-class
// context: unlike a noexcept-specifier or default argument, it sees only
// the members declared before it ([basic.scope.class]).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:39 error "use of undeclared identifier 'W'"
//@ diag: 21:21 error "use of undeclared identifier 'B'"
template <class T> struct Ok {
  static constexpr bool V = sizeof(T) > 1;
  constexpr int f() requires V { return 1; }
  constexpr int f() requires (!V) { return 2; }
};
static_assert(Ok<int>{}.f() == 1 && Ok<char>{}.f() == 2);

template <class T> struct Later {
  void f() requires (sizeof(T) > 0 && W) {}
  static constexpr bool W = true;
};

struct NonTemplate {
  void g() requires B;
  static constexpr bool B = true;
};
