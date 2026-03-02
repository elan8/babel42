#!/usr/bin/env bash
# Fetch Babel42 test fixture repositories (cloned into tests/fixtures/, gitignored)
# Run from: products/babel42/oss or repo root

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
FIXTURES_DIR="$SCRIPT_DIR/../tests/fixtures"
mkdir -p "$FIXTURES_DIR"

fixtures=(
    "moveit2_robot_arm|https://github.com/AmeyaB2005/4DOF_Robotic_Arm_In_Gazebo_With_Moveit2.git|226c0f763c0347fce925e27022ee6572cd26533c"
    "examples|https://github.com/ros2/examples.git|503696eadbdaf881d36f27e26434b0bc4109ac4e"
    "demos|https://github.com/ros2/demos.git|865426ff5d91fbd8a5761a8de637af9a5605d14d"
    "Universal_Robots_ROS2_Driver|https://github.com/PickNikRobotics/Universal_Robots_ROS2_Driver.git|f4eac2d60491ceaf1bc6cf1f78589471851c11fd"
    "tutorials|https://github.com/ros2/tutorials.git|34a1ab4c66fdf1d62c3b86639fce00969502bd0d"
    "ament_cmake|https://github.com/ament/ament_cmake.git|d4781cd07fd083c202feeaf1bfa5ddec860d5120"
    "ros2_control|https://github.com/ros-controls/ros2_control.git|4c7228fdd0edf40acf0316a2949070bc9d303bea"
    "navigation2|https://github.com/ros-navigation/navigation2.git|cf36cc8237f46f58cec1f868d2e3ad354be67092"
    "slam_toolbox|https://github.com/SteveMacenski/slam_toolbox.git|3661355d9560cd086339d6cd4738915734c7b939"
    "moveit2|https://github.com/ros-planning/moveit2.git|848c062a0454b104b72cb038d641a9eb0531f317"
)

for entry in "${fixtures[@]}"; do
    IFS='|' read -r name url ref <<< "$entry"
    dest="$FIXTURES_DIR/$name"
    if [ -d "$dest/.git" ]; then
        echo "Skipping $name (already cloned)"
        continue
    fi
    [ -d "$dest" ] && rm -rf "$dest"
    echo "Cloning $name..."
    git clone --depth 500 "$url" "$dest"
    (cd "$dest" && git fetch origin "$ref" && git checkout "$ref")
done

echo "Done. Fixtures in $FIXTURES_DIR"
