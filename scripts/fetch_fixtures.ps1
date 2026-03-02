# Fetch Babel42 test fixture repositories (cloned into tests/fixtures/, gitignored)
# Run from: products/babel42/oss or repo root

$ErrorActionPreference = "Continue"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$FixturesDir = [IO.Path]::GetFullPath((Join-Path $ScriptDir "..\tests\fixtures"))

if (-not (Test-Path $FixturesDir)) {
    New-Item -ItemType Directory -Path $FixturesDir -Force | Out-Null
}

$fixtures = @(
    @{
        name = "moveit2_robot_arm"
        url  = "https://github.com/AmeyaB2005/4DOF_Robotic_Arm_In_Gazebo_With_Moveit2.git"
        ref  = "226c0f763c0347fce925e27022ee6572cd26533c"
    },
    @{
        name = "examples"
        url  = "https://github.com/ros2/examples.git"
        ref  = "503696eadbdaf881d36f27e26434b0bc4109ac4e"
    },
    @{
        name = "demos"
        url  = "https://github.com/ros2/demos.git"
        ref  = "865426ff5d91fbd8a5761a8de637af9a5605d14d"
    },
    @{
        name = "Universal_Robots_ROS2_Driver"
        url  = "https://github.com/PickNikRobotics/Universal_Robots_ROS2_Driver.git"
        ref  = "f4eac2d60491ceaf1bc6cf1f78589471851c11fd"
    },
    @{
        name = "tutorials"
        url  = "https://github.com/ros2/tutorials.git"
        ref  = "34a1ab4c66fdf1d62c3b86639fce00969502bd0d"
    },
    @{
        name = "ament_cmake"
        url  = "https://github.com/ament/ament_cmake.git"
        ref  = "d4781cd07fd083c202feeaf1bfa5ddec860d5120"
    },
    @{
        name = "ros2_control"
        url  = "https://github.com/ros-controls/ros2_control.git"
        ref  = "4c7228fdd0edf40acf0316a2949070bc9d303bea"
    },
    @{
        name = "navigation2"
        url  = "https://github.com/ros-navigation/navigation2.git"
        ref  = "cf36cc8237f46f58cec1f868d2e3ad354be67092"
    },
    @{
        name = "slam_toolbox"
        url  = "https://github.com/SteveMacenski/slam_toolbox.git"
        ref  = "3661355d9560cd086339d6cd4738915734c7b939"
    },
    @{
        name = "moveit2"
        url  = "https://github.com/ros-planning/moveit2.git"
        ref  = "848c062a0454b104b72cb038d641a9eb0531f317"
    }
)

foreach ($f in $fixtures) {
    $dest = Join-Path $FixturesDir $f.name
    if ((Test-Path $dest) -and (Test-Path (Join-Path $dest ".git"))) {
        Write-Host "Skipping $($f.name) (already cloned)"
        continue
    }
    if (Test-Path $dest) {
        Remove-Item -Recurse -Force $dest
    }
    Write-Host "Cloning $($f.name)..."
    $null = git clone --depth 500 $f.url $dest 2>&1
    if ($LASTEXITCODE -ne 0) { throw "git clone failed for $($f.name)" }
    Push-Location $dest
    $null = git fetch origin $f.ref 2>&1
    $null = git checkout $f.ref 2>&1
    Pop-Location
    if ($LASTEXITCODE -ne 0) { throw "git checkout failed for $($f.name)" }
}

Write-Host "Done. Fixtures in $FixturesDir"
