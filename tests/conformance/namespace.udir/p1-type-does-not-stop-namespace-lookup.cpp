// [basic.lookup.udir], [namespace.udir]/1: the namespace-name in a
// using-directive or namespace-alias-definition is looked up considering only
// namespaces, so a class of the same name in an inner scope does not stop the
// search before the namespace's scope; nor does an injected-class-name. (A name
// in such a directive's nested-name-specifier is not yet restricted likewise.)
// Reduced from real code: `using namespace Request;` inside a
// namespace that has a class `Request`, reaching the namespace `Request`
// through `using namespace Vendor::Product;`.
//@ expect: compile-ok
//@ strict-diags
namespace Vendor {
namespace Product {
namespace Request {
namespace Communicators {
struct C {};
} // namespace Communicators
} // namespace Request
} // namespace Product
} // namespace Vendor

namespace App {
struct Request {};
namespace svc {
using namespace Vendor;
using namespace Product;
using namespace Request;
using namespace Communicators;
namespace req = Request;
C c;
} // namespace svc
} // namespace App

namespace X {
struct D {};
} // namespace X
namespace M {
struct X {
  void f() {
    using namespace X; // past the injected-class-name and M::X
    D d;
  }
};
} // namespace M
