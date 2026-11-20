// [temp.deduct.general]/8, [dcl.array]/1: an array bound that is not a
// converted constant expression makes the substituted type invalid. A
// variable length array (a C++ extension elsewhere in a function) is never
// formed in a SFINAE context (clang `BuildArrayType`,
// `err_vla_in_sfinae`), so the requirement is unsatisfied and the
// candidate is dropped. A standard library's "statically sized range"
// concept tests a range's size this way; reduced from that (WP-1745).
//@ expect: compile-ok

template <class T> int dynamic_size();
template <class T> constexpr int fixed_size() { return 3; }

template <class T>
concept statically_sized = requires { static_cast<char (*)[fixed_size<T>() >= 0]>(nullptr); };
template <class T>
concept dynamically_sized = requires { static_cast<char (*)[dynamic_size<T>() >= 0]>(nullptr); };

static_assert(statically_sized<int>);
static_assert(!dynamically_sized<int>);

// The same bound in a default template argument drops the candidate.
template <class T, class = char[dynamic_size<T>() + 1]> constexpr int pick(int) { return 1; }
template <class T> constexpr int pick(long) { return 2; }
static_assert(pick<int>(0) == 2);

template <class T, class = char[fixed_size<T>() + 1]> constexpr int pick2(int) { return 1; }
template <class T> constexpr int pick2(long) { return 2; }
static_assert(pick2<int>(0) == 1);

int main() {}
