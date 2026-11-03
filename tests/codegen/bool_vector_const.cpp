// [WP-450] Constant-evaluated `bool` ext vectors (clang ext_vector_type;
// [expr.const]; clang ConstExprEmitter / APValue::Vector): a constexpr or
// constant-initialized global of `bool` ext vector type is emitted as a
// folded `<N x i1>` constant (no `__cxx_global_var_init`), also as a
// struct member and an array element, and a constant `bool` vector
// expression inside a function folds to a stored constant.
typedef bool b3 __attribute__((ext_vector_type(3)));
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b12 __attribute__((ext_vector_type(12)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef bool b24 __attribute__((ext_vector_type(24)));
typedef int i8v __attribute__((ext_vector_type(8)));

struct S { int x; b8 m; b3 t; };

constexpr b3 T = {true, false, true};
constexpr b4 F = {false, true, true, false};
constexpr b8 A = {true, false, true, true, false, false, true, false};
constexpr b8 NA = !A;
constexpr b8 X = A ^ (b8)true;
constexpr b12 U = {1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1};
constexpr b16 W = __builtin_bit_cast(b16, (unsigned short)0xA55A);
constexpr b24 V = {true};
constexpr i8v I = {1, 2, 3, 4, 5, 6, 7, 8};
constexpr b8 C = __builtin_convertvector(I & 1, b8);
b8 G = A & ~(b8){false, true, false, true, false, true, false, true};
constexpr S s = {7, {true}, {false, true, true}};
constexpr b8 arr[2] = {{true}, {false, true}};

const void *p[] = {&T, &F, &A, &NA, &X, &U, &W, &V, &C, &G, &s, &arr};

b8 masked(b8 x) { return x & A; }

b16 folded() {
  constexpr b16 k = __builtin_bit_cast(b16, (unsigned short)0x00FF);
  return k;
}

// A constant `bool` vector stored to a local: the lane-resizing shuffle
// and the `bitcast` to the `iN` memory type fold (llvm
// `ConstantFoldShuffleVectorInstruction`, `ConstantFoldCastInstruction`).
void use(const void *);
void local3() { constexpr b3 k = {true, false, true}; use(&k); }
void local8() { constexpr b8 z = {}; b8 o = (b8)true; use(&z); use(&o); }
void local12() { constexpr b12 k = {1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1}; use(&k); }
