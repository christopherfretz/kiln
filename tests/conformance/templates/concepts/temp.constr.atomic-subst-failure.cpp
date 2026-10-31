//@ expect: compile-ok
// [temp.constr.atomic]/3: substitution failure in an atomic constraint makes
// it not satisfied (not an error).
template <class T> concept HasValue = T::value == 1;
struct One { static constexpr int value = 1; };
struct Two { static constexpr int value = 2; };
static_assert(HasValue<One>);
static_assert(!HasValue<Two>);
static_assert(!HasValue<int>);
