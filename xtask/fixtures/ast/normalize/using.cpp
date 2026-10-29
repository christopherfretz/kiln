namespace N { int x; typedef int I; }
namespace N { int y; }
inline namespace V { int w; }
namespace { int u; }
namespace A::B { int t; }
using namespace N;
namespace R = A::B;
namespace R2 = R;
using N::x, N::I;
enum class E { P };
void g() { using enum E; namespace L = N; I k = (int)P + x + L::y + R2::t; }
