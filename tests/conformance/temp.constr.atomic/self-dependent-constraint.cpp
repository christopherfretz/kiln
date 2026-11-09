// [temp.constr.atomic]/3, [over.match.funcs]: checking the constraint of
// Movable's converting constructor template asks __is_constructible(Movable,
// T), which considers that same constructor again; the satisfaction check
// depends on itself, so it is not satisfied (clang:
// err_constraint_depends_on_self) and the static_assert fails. kiln looped
// without bound before WP-912 (clang's SemaCXX/
// overload-resolution-deferred-templates.cpp timed out); clang's
// "satisfaction of constraint ... depends on itself" error at the
// requires-clause is not reported yet (it is raised inside the trait's
// SFINAE context).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 20:15 error "static assertion failed"
struct Movable {
  template <typename T>
    requires __is_constructible(Movable, T)
  explicit Movable(T op) noexcept;
  Movable(Movable &&) noexcept = default;
};
static_assert(__is_constructible(Movable, int));
