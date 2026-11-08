#include <iterator>
#include <vector>
#include <list>
#include <deque>
#include <sstream>
int iter_sweep(std::vector<int>& v, std::list<int>& l, std::deque<int>& d) {
  std::reverse_iterator<int*> r(v.data());
  auto p = r.operator->();
  auto q = std::prev(v.end());
  auto q2 = std::next(l.begin(), 1);
  auto q3 = std::prev(l.end());
  std::advance(q2, 1);
  auto s = std::to_address(v.begin());
  auto ri = std::make_reverse_iterator(v.begin());
  auto mi = std::make_move_iterator(v.begin());
  auto bi = std::back_inserter(v);
  *bi = 3;
  auto fi = std::front_inserter(d);
  *fi = 4;
  auto ii = std::inserter(l, l.begin());
  *ii = 5;
  std::counted_iterator<int*> ci(v.data(), 2);
  std::common_iterator<std::counted_iterator<int*>, std::default_sentinel_t> co(ci);
  std::istringstream is("1 2");
  std::istream_iterator<int> isi(is), ise;
  std::ostringstream os;
  std::ostream_iterator<int> osi(os, ",");
  *osi = 1;
  auto n = std::ranges::distance(v);
  auto nx = std::ranges::next(v.begin());
  std::ranges::advance(nx, 1);
  return *p + *q + *s + (int)std::distance(v.begin(), v.end()) + *ri + *mi + *q2 + *q3
         + *ci + *co + *isi + (int)n + *nx + (int)std::size(v) + (int)std::ssize(v)
         + (std::empty(v) ? 1 : 0) + *std::data(v) + *std::begin(v) + *std::cend(v) + (isi == ise);
}
