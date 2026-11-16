// [temp.deduct.general]/5 (CWG2369) as clang 22 and GCC order it: a
// non-dependent conversion is checked before the associated constraints
// unless it may use a user-defined conversion, which means the target
// class has a constructor other than copy or move that takes one
// argument, or the source class has a conversion function (through a
// reference, and through one level of pointers on both sides). So a
// tag type without converting constructors drops a candidate before its
// constraint is checked (no recursive satisfaction, no instantiation of
// an incomplete class), while a converting constructor delays the
// conversion and the failed constraint is what the note reports. Found
// running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 49:3 error "no matching function for call to 'via_ctor'"
//@ diag: 44:43 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 44:29 note "because 'small<int>' evaluated to false"
//@ diag: 50:3 error "no matching function for call to 'via_plain'"
//@ diag: 45:43 note "candidate function template not viable: no known conversion from 'std::nullptr_t' to 'plain' for 1st argument"
//@ diag: 51:3 error "no matching function for call to 'via_ptr'"
//@ diag: 46:43 note "candidate function template not viable: no known conversion from 'from_int *' to 'plain_ptr *' for 1st argument"
//@ diag: 52:3 error "no matching function for call to 'via_ctor_ptr'"
//@ diag: 47:43 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 47:29 note "because 'small<int>' evaluated to false"
struct red {};
struct blue {};
template <class T> concept paints = requires(T v) { paint(red{}, v); };
template <class T> void paint(red, T);
template <class T> void paint(blue, T) requires paints<T>;
void ok1() { paint(red{}, 0); paint(blue{}, 0); }

struct Hole;
template <class T> struct Box { T t; };
template <class T> concept boxable = sizeof(Box<T>) > 0;
struct tag { explicit tag() = default; };
template <boxable T> void put(tag, T *);
void put(...);
void ok2(Hole *p) { put(0, p); }

template <class T> constexpr bool small = sizeof(T) < 2;
struct from_int { from_int(int); };
struct plain {};
struct plain_ptr {};
template <class T> requires small<T> void via_ctor(from_int, T);
template <class T> requires small<T> void via_plain(plain, T);
template <class T> requires small<T> void via_ptr(plain_ptr *, T);
template <class T> requires small<T> void via_ctor_ptr(from_int *, T);
void bad() {
  via_ctor(nullptr, 1);
  via_plain(nullptr, 1);
  via_ptr((from_int *)nullptr, 1);
  via_ctor_ptr((plain *)nullptr, 1);
}
