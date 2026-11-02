// [expr.throw]/5, [except.throw]/4 (C++26, P3068R6) + kiln Oven budgets
// (spec/oven.md "Exceptions"): when the constructor of the exception
// object throws, the object never exists and its storage is released; the
// constructor's own exception propagates instead. 10000 rounds where every
// other exception object's constructor throws: neither the transient heap
// nor the arena grows with the number of rounds.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ args: -fconstexpr-steps=100000000
//@ oven: 41 allocs<=15000 peak_bytes<=64 peak_arena<=256
//@ mem<=512
struct Err {
  long long payload[4];
  constexpr explicit Err(int i) : payload{i, i, i, i} {
    if (i % 2 != 0) throw i;
  }
};

constexpr long long churn(int rounds) {
  long long sum = 0;
  for (int i = 0; i < rounds; ++i) {
    try {
      throw Err(i);
    } catch (const Err &e) {
      sum += e.payload[3];
    } catch (int odd) {
      sum -= odd;
    }
  }
  return sum;
}

constexpr bool nested() {
  try {
    throw Err(1);
  } catch (int e) {
    return e == 1;
  }
}

// 0 + 2 + ... + 9998 minus 1 + 3 + ... + 9999.
static_assert(churn(10000) == -5000);
static_assert(nested());

int main() {}
