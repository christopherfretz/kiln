// [dcl.enum]/3 (C++11, N2764): opaque enum declarations with a fixed
// underlying type, completed later; uses before completion. WP-646 probe.
//@ expect: compile-ok
//@ std: c++11
//@ strict-diags
enum E : unsigned char;
enum class S;
enum class T : long long;
struct Holder { E e; S s; T t; };
static_assert(sizeof(Holder::e) == 1, "E is complete enough to size");
static_assert(sizeof(S) == sizeof(int), "scoped default underlying is int");
E make(unsigned char c) { return static_cast<E>(c); }
enum E : unsigned char { a, b = 255 };
enum class S { x, y };
enum class T : long long { big = 1LL << 40 };
static_assert(static_cast<long long>(T::big) == (1LL << 40), "fixed type");
static_assert(b == 255 && static_cast<int>(S::y) == 1, "values");
struct Outer { enum class In : short; };
enum class Outer::In : short { v = -1 };
static_assert(static_cast<short>(Outer::In::v) == -1, "member opaque enum");
