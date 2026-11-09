//@ args: -fconstexpr-steps=200000000
// A dense switch in a hot loop (a bytecode interpreter's dispatch): the
// Oven's switch dispatch cost per iteration, with every case label a direct
// child of the switch body. WP-1144 lowered labels nested in loops and
// blocks (Duff's device) to jumps; this entry guards that ordinary switches
// did not pay for it.

constexpr unsigned run(unsigned n) {
  unsigned acc = 1, op = 0;
  for (unsigned i = 0; i < n; ++i) {
    switch (op) {
    case 0: acc += i; break;
    case 1: acc ^= i << 3; break;
    case 2: acc *= 3; break;
    case 3: acc -= i >> 1; break;
    case 4: acc += acc >> 7; break;
    case 5: acc ^= 0x5bd1e995u; break;
    case 6: acc = acc * 5 + 1; break;
    case 7: acc += 17; break;
    default: acc = 0; break;
    }
    op = (op + (acc & 3) + 1) & 7;
  }
  return acc;
}

static_assert(run(10) == 32);
static_assert(run(2000000) == 1172205115);
