// [temp.deduct.general]/8: only an invalid type or expression makes
// substitution fail. A warning that -Werror turns into an error is still
// a warning there (clang's SFINAE response follows the diagnostic's
// class, not its mapping): the requirement is satisfied.
//@ args: -Werror -Wall
//@ strict-diags
struct S { [[deprecated]] int f(); };

template <class T> concept C = requires(T t) { t.f(); };
static_assert(C<S>);

