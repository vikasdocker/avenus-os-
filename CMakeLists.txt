cmake_minimum_required(VERSION 3.22)

project(
  AetherOS
  VERSION 0.1.0
  DESCRIPTION "Project Genesis Aether OS native system components"
  LANGUAGES C CXX
)

option(AETHER_STATIC "Link native C binaries statically where supported" OFF)
option(AETHER_BUILD_UI "Build the Qt/QML shell" OFF)
option(AETHER_WARNINGS_AS_ERRORS "Treat C and C++ compiler warnings as errors" ON)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
set(CMAKE_C_EXTENSIONS OFF)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

include(CTest)

function(aether_apply_c_warnings target_name)
  if(MSVC)
    target_compile_options(${target_name} PRIVATE /W4)
    if(AETHER_WARNINGS_AS_ERRORS)
      target_compile_options(${target_name} PRIVATE /WX)
    endif()
  else()
    target_compile_options(
      ${target_name}
      PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Wconversion
        -Wshadow
        -Wstrict-prototypes
    )
    if(AETHER_WARNINGS_AS_ERRORS)
      target_compile_options(${target_name} PRIVATE -Werror)
    endif()
  endif()
endfunction()

function(aether_apply_cxx_warnings target_name)
  if(MSVC)
    target_compile_options(${target_name} PRIVATE /W4)
    if(AETHER_WARNINGS_AS_ERRORS)
      target_compile_options(${target_name} PRIVATE /WX)
    endif()
  else()
    target_compile_options(
      ${target_name}
      PRIVATE
        -Wall
        -Wextra
        -Wpedantic
        -Wconversion
        -Wshadow
    )
    if(AETHER_WARNINGS_AS_ERRORS)
      target_compile_options(${target_name} PRIVATE -Werror)
    endif()
  endif()
endfunction()

function(aether_apply_static_link target_name)
  if(AETHER_STATIC AND NOT MSVC)
    target_link_options(${target_name} PRIVATE -static)
  endif()
endfunction()

add_subdirectory(system/aether-healthd)
add_subdirectory(system/aetherctl)

if(AETHER_BUILD_UI)
  add_subdirectory(ui/shell)
endif()

