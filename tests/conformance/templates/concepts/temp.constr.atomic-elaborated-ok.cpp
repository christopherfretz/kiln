//@ expect: compile-ok
// [temp.constr.atomic]/3: the atomic constraint's value after substitution.
template <int N> concept Even = N % 2 == 0;
static_assert(Even<4>);
static_assert(!Even<3>);
template <int N> requires Even<N> constexpr int half = N / 2;
static_assert(half<10> == 5);
