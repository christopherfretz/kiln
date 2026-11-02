// [lex.ext]/3-8, [over.literal]: a user-defined-literal is looked up as an
// unqualified name, so literal operators made visible by using-directives
// are found transitively through nested inline namespaces and through a
// directive in a namespace that re-exports them (libc++ <chrono> shape:
// `std::literals::chrono_literals` and `std::chrono`). The `d` and `f`
// suffixes are user literal suffixes here, not floating-point ones.
//@ strict-diags
namespace N {
  struct day { unsigned v; };
  inline namespace literals {
    inline namespace chrono_literals {
      constexpr day operator""_d(unsigned long long x) { return day{unsigned(x)}; }
      constexpr long double operator""_f(long double x) { return x; }
    }
  }
  namespace chrono { using namespace literals::chrono_literals; }
}
constexpr unsigned t1() { using namespace N::chrono; return (4_d).v; }
constexpr unsigned t2() { using namespace N::literals; return (5_d).v; }
constexpr unsigned t3() { using namespace N::literals::chrono_literals; return (6_d).v; }
constexpr unsigned t4() { using namespace N; return (7_d).v; }
static_assert(t1() == 4 && t2() == 5 && t3() == 6 && t4() == 7);
constexpr bool t5() { using namespace N::chrono; return 1.5_f == 1.5L; }
static_assert(t5());
