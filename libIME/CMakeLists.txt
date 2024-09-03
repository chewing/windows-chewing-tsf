find_package(Corrosion QUIET)
if(NOT Corrosion_FOUND)
    FetchContent_Declare(
        Corrosion
        GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
        GIT_TAG 64289b1d79d6d19cd2e241db515381a086bb8407 # v0.5
        FIND_PACKAGE_ARGS
    )
    FetchContent_MakeAvailable(Corrosion)
endif()

corrosion_import_crate(MANIFEST_PATH ../rustlib/Cargo.toml CRATES rustlib CRATE_TYPES staticlib)
corrosion_add_cxxbridge(rustlib_bridge
        CRATE rustlib
        FILES lib.rs
)
corrosion_add_target_rustflags(rustlib "-C target-feature=+crt-static")

if(CMAKE_CXX_COMPILER_ID MATCHES "Clang" AND CMAKE_CXX_SIMULATE_ID MATCHES "MSVC")
    corrosion_set_env_vars(rustlib "CFLAGS=-EHsc" "CXXFLAGS=-EHsc")
endif()

add_library(libIME_static STATIC
    # Core TSF part
    ImeModule.cpp
    ImeModule.h
    libIME.cpp
    libIME.h
    TextService.cpp
    TextService.h
    KeyEvent.cpp
    KeyEvent.h
    EditSession.cpp
    EditSession.h
    DisplayAttributeInfo.cpp
    DisplayAttributeInfo.h
    DisplayAttributeInfoEnum.cpp
    DisplayAttributeInfoEnum.h
    DisplayAttributeProvider.cpp
    DisplayAttributeProvider.h
    LangBarButton.cpp
    LangBarButton.h
    Utils.cpp
    Utils.h
    # GUI-related code
    DrawUtils.h
    DrawUtils.cpp
    Window.cpp
    Window.h
    ImeWindow.cpp
    ImeWindow.h
    MessageWindow.cpp
    MessageWindow.h
    CandidateWindow.h
    CandidateWindow.cpp
    NinePatch.h
    NinePatch.cpp
)

target_link_libraries(libIME_static
    PUBLIC rustlib_bridge
    PUBLIC shlwapi.lib
    PUBLIC d2d1.lib
    PUBLIC d3d11.lib
    PUBLIC dwrite.lib
    PUBLIC dcomp.lib
)