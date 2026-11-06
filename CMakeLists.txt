# CMake raíz (Fase 0). Propietario posterior: agente G (B10 · Plataforma).
cmake_minimum_required(VERSION 3.20)
project(JuegoDeBaile LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

option(JD_WITH_SDL3  "Compilar el adaptador real de Joy-Con con SDL3" OFF)
option(JD_WITH_GODOT "Compilar la GDExtension (requiere godot-cpp)" OFF)
option(JD_BUILD_TESTS "Compilar tests" ON)

if(JD_BUILD_TESTS)
  enable_testing()
endif()

include_directories(${CMAKE_SOURCE_DIR}/contracts ${CMAKE_SOURCE_DIR}/engine/common)

# Cada área del motor define su propia biblioteca estática jd_<area> en su CMakeLists.
add_subdirectory(engine/common)
foreach(area input session signal features choreo judge)
  if(EXISTS ${CMAKE_SOURCE_DIR}/engine/${area}/CMakeLists.txt)
    add_subdirectory(engine/${area})
  endif()
endforeach()
add_subdirectory(engine/api)
if(JD_BUILD_TESTS AND EXISTS ${CMAKE_SOURCE_DIR}/engine/tests/CMakeLists.txt)
  add_subdirectory(engine/tests)
endif()
if(JD_WITH_GODOT AND EXISTS ${CMAKE_SOURCE_DIR}/game/gdextension/CMakeLists.txt)
  add_subdirectory(game/gdextension)
endif()
