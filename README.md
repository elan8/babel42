# Babel42 — ROS2 Project Analysis Tool

Babel42 is an analysis tool for ROS2 projects, with an open-source CLI edition and a Pro edition that builds on the same core.

## Crate Structure

- **babel42-core**: Shared library for ROS2 project analysis (package discovery, package.xml parsing, workspace structure). Used by both OSS and Pro editions.
- **babel42**: Open-source CLI with basic ROS2 project analysis.
- **babel42-pro**: Pro CLI with additional features, built on babel42-core.

## Installation

```bash
cd products/babel42
cargo install --path babel42
```

## Build and Test

From `products/babel42`:

```bash
cargo build
cargo test
cargo test -p babel42-core
```

## Usage

```bash
# Analyze a ROS2 workspace
babel42 analyze <path>

# Check for issues (CI-friendly)
babel42 check <path>

# Export project model to JSON
babel42 export <path> --format json
```

Or via cargo run:

```bash
cargo run -p babel42 -- analyze <path>
cargo run -p babel42 -- check <path>
```

## CI Integration

Example GitHub Actions workflow:

```yaml
- name: Babel42 check
  run: |
    cargo install --path products/babel42/babel42
    babel42 check . --fail-on error
```

See [docs/RULES.md](docs/RULES.md) for the full rule reference.

## Test Fixtures

- **sample_workspace**: Minimal synthetic workspace (`tests/fixtures/sample_workspace`)
- **moveit2_robot_arm**: Robot arm with MoveIt2 from [AmeyaB2005/4DOF_Robotic_Arm_In_Gazebo_With_Moveit2](https://github.com/AmeyaB2005/4DOF_Robotic_Arm_In_Gazebo_With_Moveit2):
  - **4-DOF robotarm** in Gazebo, bestuurd met MoveIt2
  - **5 packages**: arduinobot_bringup, arduinobot_controller, arduinobot_description, arduinobot_moveit, arduinobot_msgs
  - **MoveIt2** + Gazebo + ros2_control, custom msgs, launch files

Run `scripts/fetch_fixtures.ps1` (Windows) or `scripts/fetch_fixtures.sh` (Linux/macOS) to clone fixture repos for integration tests.

## Rules Reference

See [docs/RULES.md](docs/RULES.md) for all checks, severities, and fix hints.
