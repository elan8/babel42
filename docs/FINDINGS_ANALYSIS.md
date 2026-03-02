# Babel42 Findings Analysis

Analyse van alle dep-errors uit de integratietests: echte fouten vs. babel42-bugs (false positives).

## Geïmplementeerde fixes (babel42-bugs)

| Bug | Oorzaak | Fix |
|-----|---------|-----|
| `NO_CMAKE_PACKAGE_REGISTRY` | CMake find_package-optie werd als package geparsed | Toegevoegd aan skip_keywords in cmake/mod.rs |
| `ament_cmake_core` | Transitive build dep van ament_cmake | Toegevoegd aan is_build_tool_or_skippable |

---

## Per fixture

### demos (3 errors)

| Package | Finding | Verdict |
|---------|---------|---------|
| demo_nodes_cpp_native, dummy_map_server, dummy_sensors | find_package(rmw) not in package.xml | **Echte error** – rmw is een ROS2-package; ontbreekt in package.xml (wel rmw_fastrtps_cpp) |

### Universal_Robots_ROS2_Driver (3 errors)

| Package | Finding | Verdict |
|---------|---------|---------|
| ur_controllers | ament_cmake_gmock | **Echte error** – test-dep, hoort in test_depend |
| ur_dashboard_msgs | builtin_interfaces | **Echte error** – msg-dependency ontbreekt |
| ur_robot_driver | rclcpp_action | **Echte error** – actie-client dependency ontbreekt |

### navigation2 (22 errors)

| Type | Packages | Verdict |
|------|----------|---------|
| ament_cmake_gtest | nav2_amcl, opennav_docking_bt | **Echte error** – test-dep |
| yaml-cpp, bond, bondcpp, std_msgs, nav_msgs, nav2_ros_common, rclcpp_lifecycle, rclcpp_components, rosidl_default_generators | diverse | **Echte error** – dependencies ontbreken |
| GRAPHICSMAGICKCPP | nav2_map_server | **Grensgeval** – systeemlib, vaak via rosdep |
| nanoflann, nlohmann_json | nav2_route, nav2_smac_planner | **Grensgeval** – vendor libs via rosdep |
| angles, behaviortree_cpp, pluginlib | nav2_system_tests | **Echte error** – ontbrekende deps |

### moveit2 (20 errors)

| Type | Verdict |
|------|---------|
| kdl_parser, orocos_kdl, random_numbers, rclcpp [moveit_kinematics] | **Echte error** – package.xml onvolledig |
| moveit_ros_warehouse, tf2, warehouse_ros [moveit_ros_trajectory_cache] | **Echte error** – warehouse-stack |
| moveit_ros_planning, moveit_msgs, moveit_core, octomap_msgs, rclcpp_action, rviz_* [moveit_ros_visualization] | **Echte error** – veel directe deps |
| moveit_common [moveit_setup_*] | **Echte error** – moveit_common als build-dep |

MoveIt gebruikt `moveit_package()` maar injecteert geen find_package-deps; de packages hebben zelf incomplete package.xml.

### ros2_control (1 error)

| Package | Finding | Verdict |
|---------|---------|---------|
| controller_manager | ament_cmake_core | **Gefixed** – nu in is_build_tool_or_skippable |

### slam_toolbox (4 errors → 3 na fix)

| Package | Finding | Verdict |
|---------|---------|---------|
| karto_sdk | NO_CMAKE_PACKAGE_REGISTRY | **Gefixed** – CMake-optie, geen package |
| karto_sdk | rclcpp, rclcpp_lifecycle | **Echte error** – karto_sdk is root-as-package, package.xml mogelijk onvolledig |
| karto_sdk | TBB | **Grensgeval** – systeemlib, vaak via rosdep |

---

## Warnings (geen errors)

| Fixture | Finding | Verdict |
|---------|---------|---------|
| moveit2_robot_arm | arduinobot_remote not in workspace | **Waarschijnlijk terecht** – remote package buiten workspace |
| Universal_Robots | topic_no_publisher (io_and_status_controller/*) | **Grensgeval** – controller publiceert mogelijk pas at runtime |
| navigation2 | manifest/missing_description | **Terecht** – package.xml mist description |

---

## Samenvatting

- **Babel42-bugs (opgelost):** 2 (NO_CMAKE_PACKAGE_REGISTRY, ament_cmake_core)
- **Echte errors:** ~45 – incomplete package.xml in upstream repos
- **Grensgevallen:** system/vendor libs (yaml-cpp, TBB, nanoflann, nlohmann_json) – vaak via rosdep; strikt genomen horen ze wel in package.xml
