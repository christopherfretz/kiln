enum Small : char { S0 };
enum Big : unsigned long long { B0 };
enum Plain { P0 };
struct E { Small s; Big b; Plain p; enum class Inner : short { I } inner; };
