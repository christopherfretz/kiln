# daw_json_link (WP-792): included after `project(daw-json-link)` through
# -DCMAKE_PROJECT_daw-json-link_INCLUDE. With DAW_ENABLE_TESTING its
# extern/CMakeLists.txt fetches header_libraries at the moving branch `v2`,
# whose later releases (v2.136+) dropped the `_u32`/`_u64` literals v3.38.1
# uses, so every TU fails with every compiler. FetchContent keeps the first
# declaration of a name: declare it here at v2.135.0, the release current
# when v3.38.1 was tagged.
include(FetchContent)
FetchContent_Declare(
  daw_header_libraries
  GIT_REPOSITORY https://github.com/beached/header_libraries.git
  GIT_TAG v2.135.0
  GIT_SHALLOW 1
)
