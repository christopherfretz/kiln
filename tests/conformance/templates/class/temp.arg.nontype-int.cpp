//@ expect: ast
//@ oracle: clang
// [temp.arg.nontype]/1: a non-type template argument is a converted constant
// expression of the parameter's type.
template <int N> struct Arr { char data[N]; };
static_assert(sizeof(Arr<3>) == 3);
static_assert(sizeof(Arr<1 + 4>) == 5);
