// [over.match.list]/1.2: when no initializer-list constructor is viable,
// all constructors are candidates with the list's elements as arguments.
#include "Inputs/initializer_list.h"
struct U {
  int how;
  constexpr U(std::initializer_list<int*>) : how(1) {}
  constexpr U(int, int) : how(2) {}
};
static_assert(U{1, 2}.how == 2);
static_assert(U{nullptr}.how == 1);
constexpr U u = {3, 4};
static_assert(u.how == 2);
// A narrowing element still selects the initializer-list constructor
// (and is then ill-formed); a non-convertible one does not.
struct S {
  int how;
  constexpr S(std::initializer_list<char>) : how(1) {}
  constexpr S(const char*, int) : how(2) {}
};
static_assert(S{"x", 1}.how == 2);
static_assert(S{'a', 'b'}.how == 1);
