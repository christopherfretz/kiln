// [temp.arg.explicit]/4: trailing template arguments are deduced.
template <class To, class From> constexpr To conv(From f) { return static_cast<To>(f); }
static_assert(conv<int>(3.9) == 3);
static_assert(conv<char>(65) == 'A');
static_assert(conv<long, int>(5) == 5);
