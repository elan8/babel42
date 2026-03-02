# Babel42 test fixtures

These workspaces are used to validate babel42 on real-world ROS2 projects.

| Project | Repo | Purpose |
|---------|------|---------|
| moveit2_robot_arm | AmeyaB2005/4DOF_Robotic_Arm_In_Gazebo_With_Moveit2 | Robot arm with MoveIt2, Gazebo |
| examples | ros2/examples | Official ROS2 example packages |
| demos | ros2/demos | ROS2 demo nodes and compositions |
| tutorials | ros2/tutorials | rclcpp/rclpy/rclc tutorials |
| Universal_Robots_ROS2_Driver | PickNikRobotics/Universal_Robots_ROS2_Driver | Robot driver with launch, xacro, actions |
| ament_cmake | ament/ament_cmake | Core ament build infrastructure |
| navigation2 | ros-navigation/navigation2 | Nav2 navigation stack |
| moveit2 | ros-planning/moveit2 | MoveIt2 motion planning framework |
| ros2_control | ros-controls/ros2_control | Robot control framework |
| slam_toolbox | SteveMacenski/slam_toolbox | SLAM with root-as-package layout |

## Fetching fixtures

The fixture repos are **not** included in the repo (gitignored). Clone them from the repo root:

```powershell
# Windows (PowerShell)
./scripts/fetch_fixtures.ps1
```

```bash
# Linux / macOS
./scripts/fetch_fixtures.sh
```

## Running babel42

From the repo root:

```bash
# Analyze a fixture
babel42 analyze tests/fixtures/demos

# Check for issues
babel42 check tests/fixtures/demos
```

## Integration test

After fetching fixtures, run the integration test that validates all fixtures:

```bash
cargo test integration_all_fixtures_analyze_and_check
```

This test runs workspace discovery + build + check on each fixture. Fixtures that are not present (not yet fetched) are skipped.
