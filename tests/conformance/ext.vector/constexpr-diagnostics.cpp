// Vector expressions that are not constant expressions under clang 19
// ([expr.const]; VectorExprEvaluator): lane overflow, shift counts and
// division by zero are diagnosed per lane with the vector type; subscripts,
// __builtin_reduce_* and vector-to-scalar bit casts are "not valid" at the
// expression's start; a vector-condition `?:` fails without a note; an
// uninitialized vector read is diagnosed (WP-173).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 49:15 error "constexpr variable 'ovf' must be initialized by a constant expression"
//@ diag: 49:23 note "value 4294967294 is outside the range of representable values of type 'g4i' (vector of 4 'int' values)"
//@ diag: 50:15 error "constexpr variable 'dz' must be initialized by a constant expression"
//@ diag: 50:22 note "division by zero"
//@ diag: 51:15 error "constexpr variable 'rz' must be initialized by a constant expression"
//@ diag: 51:22 note "division by zero"
//@ diag: 53:15 error "constexpr variable 'fz' must be initialized by a constant expression"
//@ diag: 53:23 note "division by zero"
//@ diag: 54:15 error "constexpr variable 'sh' must be initialized by a constant expression"
//@ diag: 54:22 note "shift count 40 >= width of type 'g4i' (vector of 4 'int' values) (32 bits)"
//@ diag: 55:15 error "constexpr variable 'shn' must be initialized by a constant expression"
//@ diag: 55:23 note "negative shift count -1"
//@ diag: 56:21 error "constexpr variable 'sub' must be initialized by a constant expression"
//@ diag: 57:21 error "constexpr variable 'red' must be initialized by a constant expression"
//@ diag: 58:15 error "constexpr variable 'vc' must be initialized by a constant expression"
//@ diag: 60:15 error "constexpr variable 'evc' must be initialized by a constant expression"
//@ diag: 61:27 error "constexpr variable 'wide' must be initialized by a constant expression"
//@ diag: 63:15 error "constexpr variable 'fv' must be initialized by a constant expression"
//@ diag: 62:40 note "subexpression not valid in a constant expression"
//@ diag: 63:20 note "in call to 'f({1, 2, 3, 4}, 2)'"
//@ diag: 65:15 error "constexpr variable 'gv' must be initialized by a constant expression"
//@ diag: 64:33 note "subexpression not valid in a constant expression"
//@ diag: 65:20 note "in call to 'g({1, 2, 3, 4})'"
//@ diag: 67:15 error "constexpr variable 'hv' must be initialized by a constant expression"
//@ diag: 66:26 note "subexpression not valid in a constant expression"
//@ diag: 67:20 note "in call to 'h({1, 2, 3, 4})'"
//@ diag: 69:15 error "constexpr variable 'kv' must be initialized by a constant expression"
//@ diag: 71:15 error "constexpr variable 'mv' must be initialized by a constant expression"
//@ diag: 70:28 note "value 2147483648 is outside the range of representable values of type 'g4i' (vector of 4 'int' values)"
//@ diag: 71:20 note "in call to 'm({1, 2, 3, 4})'"
//@ diag: 73:15 error "constexpr variable 'uv' must be initialized by a constant expression"
//@ diag: 72:35 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 73:20 note "in call to 'u()'"
//@ requires: clang<22
// clang 22 no longer emits error: constexpr variable 'sub' must be initialized by a constant expres...; WP-355 residual
typedef int g4i __attribute__((vector_size(16)));
typedef float g4f __attribute__((vector_size(16)));
typedef int v4i __attribute__((ext_vector_type(4)));
constexpr g4i a = {1, 2, 3, 4};
constexpr g4i ovf = a * 0x7fffffff;
constexpr g4i dz = a / (a - 1);
constexpr g4i rz = a % (a - 2);
constexpr g4f fa = {1, 2, 3, 4};
constexpr g4f fz = fa / 0.0f;
constexpr g4i sh = a << 40;
constexpr g4i shn = a >> -1;
constexpr int sub = a[1];
constexpr int red = __builtin_reduce_add(a);
constexpr g4i vc = a > 2 ? a : -a;
constexpr v4i e = {1, 2, 3, 4};
constexpr v4i evc = e ? e : e;
constexpr __int128 wide = (__int128)a;
constexpr int f(g4i v, int i) { return v[i]; }
constexpr int fv = f(a, 2);
constexpr int g(g4i v) { return __builtin_reduce_max(v); }
constexpr int gv = g(a);
constexpr g4i h(g4i v) { v[0] = 1; return v; }
constexpr g4i hv = h(a);
constexpr g4i k(g4i v) { return v ? v : -v; }
constexpr g4i kv = k(a);
constexpr g4i m(g4i v) { v *= 0x40000000; return v; }
constexpr g4i mv = m(a);
constexpr g4i u() { g4i v; return v + 1; }
constexpr g4i uv = u();
