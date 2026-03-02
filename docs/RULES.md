# Babel42 Rule Reference

Overzicht van alle checks en hun severity.

| Rule ID | Severity | Beschrijving | Fix hint |
|---------|----------|--------------|----------|
| dep/find_package_missing | Error/Warn | find_package(X) in CMake maar niet in package.xml (Warn voor system libs) | Add \<depend\>X\</depend\> of \<test_depend\>X\</test_depend\> |
| dep/ament_target_undeclared | Error/Warn | ament_target_dependencies references package not in package.xml (Warn voor system libs) | Add \<depend\>X\</depend\> of \<test_depend\>X\</test_depend\> |
| dep/circular | Error | Circular dependency in package graph | Remove circular dependency |
| launch/include_cycle | Error | Include cycle in launch file graph (A includes B, B includes A) | Remove circular includes |
| launch/missing_package | Warn | Included package not in workspace | Add package to workspace or ensure installed |
| runtime/topic_no_publisher | Warn | Topic has subscriber(s) but no publishers | Add publisher or fix launch/topics |
| runtime/topic_no_subscriber | Info | Topic has publisher(s) but no subscribers | Informational; may be expected |
| runtime/topic_type_mismatch | Error | Topic has publisher/subscriber type mismatch | Align msg types |
| runtime/service_type_mismatch | Error | Service has server/client type mismatch | Align srv types |
| manifest/missing_description | Warn | Package has no description | Add \<description\>\</description\> |
| manifest/no_maintainer | Warn | Package has no maintainers | Add \<maintainer\>\</maintainer\> |

## Test-only packages

De volgende packages krijgen automatisch een \<test_depend\> fix hint in plaats van \<depend\>:

- ament_cmake_gtest
- ament_cmake_gmock

## System/vendor packages

De volgende packages worden nog steeds gerapporteerd bij find_package_missing of ament_target_undeclared, maar met **Warn** in plaats van Error (via rosdep/system):

- yaml-cpp, TBB, nanoflann, nlohmann_json
- graphicsmagickcpp, bond, bondcpp
