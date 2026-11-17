// sides: post
// scope: defs
// The header's templates instantiated with other arguments, explicitly
// instantiated, and their exception specifications and types queried.
inline int kc_other_args() {
  hp::Box<double> b;
  hp::Box<unsigned> c;
  return (int)b.get() + (int)hp::twice(1.5) + b.conv('c') + hp::vt<long> + hp::Box<double>::counter + (int)c.noex()
       + hp::tlam<short>() + (int)hp::tsig(2.0);
}
template struct hp::Box<float>;
template float hp::twice<float>(float);
template const long hp::vt<long>;
static_assert(noexcept(hp::Box<long>().noex()));
static_assert(noexcept(hp::Box<short>().noex()));
inline int kc_query() { return noexcept(hp::plain(1)) + sizeof(hp::deduced()) + sizeof(hp::Box<long>().conv(1.0)); }
template <> inline unsigned char hp::Box<unsigned char>::get() const { return 9; }
using hp::plain;
typedef hp::Box<int> KcBoxInt;
