//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:10 error "no matching function for call to 'f'"
//@ diag: 37:14 note "candidate function not viable: constraints not satisfied"
//@ diag: 37:27 note "because 'long' does not satisfy 'Int'"
//@ diag: 34:34 note "because '__is_same(long, int)' evaluated to false"
//@ diag: 38:14 note "candidate function not viable: constraints not satisfied"
//@ diag: 38:27 note "because 'long' does not satisfy 'IntSmall'"
//@ diag: 35:39 note "because 'long' does not satisfy 'Int'"
//@ diag: 34:34 note "because '__is_same(long, int)' evaluated to false"
//@ diag: 48:22 error "invalid reference to function 'g': constraints not satisfied"
//@ diag: 39:26 note "because 'long' does not satisfy 'Int'"
//@ diag: 34:34 note "because '__is_same(long, int)' evaluated to false"
//@ diag: 49:27 error "invalid reference to function 'h': constraints not satisfied"
//@ diag: 40:21 note "because 'long' does not satisfy 'Int'"
//@ diag: 34:34 note "because '__is_same(long, int)' evaluated to false"
//@ diag: 50:21 error "invalid reference to function 's': constraints not satisfied"
//@ diag: 41:27 note "because 'long' does not satisfy 'Int'"
//@ diag: 34:34 note "because '__is_same(long, int)' evaluated to false"
//@ diag: 51:23 error "invalid operands to binary expression ('Box<long>' and 'Box<long>')"
//@ diag: 44:15 note "candidate function not viable: constraints not satisfied"
//@ diag: 44:45 note "because 'long' does not satisfy 'Int'"
//@ diag: 34:34 note "because '__is_same(long, int)' evaluated to false"
//@ diag: 52:21 error "call to member function 'amb' is ambiguous"
//@ diag: 42:7 note "candidate function"
//@ diag: 43:7 note "candidate function"
//@ diag: 53:21 error "no matching member function for call to 'same'"
//@ diag: 45:60 note "candidate template ignored: constraints not satisfied [with U = double]"
//@ diag: 45:32 note "because 'sizeof(int) == sizeof(double)' (4 == 8) evaluated to false"
// [over.match.viable]/3, [expr.prim.id.general]/4: the trailing requires-clause
// of a member of a class template specialization (the pattern's, checked
// with the specialization's arguments) must be satisfied, whether the
// member is named through overload resolution or directly.
template <class T> concept Int = __is_same(T, int);
template <class T> concept IntSmall = Int<T> && sizeof(T) <= 4;
template <class T> struct Box {
  static int f() requires Int<T> { return 1; }
  static int f() requires IntSmall<T> { return 2; }
  int g() const requires Int<T> { return 3; }
  void h() requires Int<T>;
  static int s() requires Int<T> { return 4; }
  int amb() requires Int<T> { return 5; }
  int amb() requires (sizeof(T) == 4) { return 6; }
  friend bool operator==(Box, Box) requires Int<T> { return true; }
  template <class U> requires (sizeof(T) == sizeof(U)) int same(U) { return 7; }
};
int x1 = Box<long>::f();
int x2 = Box<long>{}.g();
void k() { Box<long> b; b.h(); }
int x3 = Box<long>::s();
bool x4 = Box<long>{} == Box<long>{};
int x5 = Box<int>{}.amb();
int x6 = Box<int>{}.same(3.0);
