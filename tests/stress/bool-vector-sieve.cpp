// [expr.const], spec/oven.md "Bool vectors" (WP-438): a sieve of
// Eratosthenes over 16384 lanes kept in 256 `bool` ext vectors of 64 lanes.
// The Oven stores a bool vector as packed bits, so the sieve frame is 256 * 8
// bytes (one byte per lane would be 16 KiB) and a lane store or load is a bit
// operation in place: no allocation per lane or per vector. The second
// evaluation builds a vector one lane at a time, ORs it into an accumulator
// and reduces it. Lane subscripts on bool vectors fold from clang 22 only.
//@ expect: run
//@ requires: clang>=22
//@ oven: 46 steps<=36000 calls<=1 allocs<=0 peak_bytes<=2304
//@ oven: 47 steps<=400 calls<=1 allocs<=0 peak_bytes<=64

typedef bool b64 __attribute__((ext_vector_type(64)));

constexpr int W = 64, R = 256, N = W * R;

struct Sieve {
  b64 w[R];
};

constexpr int count_primes() {
  Sieve s{};
  for (int i = 2; i * i < N; ++i)
    if (!s.w[i / W][i % W])
      for (int j = i * i; j < N; j += i)
        s.w[j / W][j % W] = true;
  s.w[0][0] = s.w[0][1] = true;
  int c = 0;
  for (int k = 0; k < R; ++k) {
    b64 p = !s.w[k];
    c += __builtin_popcountll(__builtin_bit_cast(unsigned long long, p));
  }
  return c;
}

constexpr bool all_marked() {
  b64 acc = (b64)false;
  for (int k = 0; k < W; ++k) {
    b64 one = (b64)false;
    one[k] = true;
    acc = acc | one;
  }
  return __builtin_reduce_and(acc) && !__builtin_reduce_or(!acc);
}

static_assert(count_primes() == 1900);
static_assert(all_marked());

int main() { return count_primes() == 1900 && all_marked() ? 0 : 1; }
