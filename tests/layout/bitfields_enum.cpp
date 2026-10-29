enum E : unsigned char { A, B };
enum class F : short { X };
enum G { G0, G1 };
struct S { E e : 2; F f : 3; G g : 4; int i : 3; };
