# Warning configuration shared by every ai-nsc target.
#
# Usage: ainsc_set_warnings(<target>)

function(ainsc_set_warnings target)
  if(MSVC)
    target_compile_options(${target} PRIVATE /W4)
    if(AINSC_ENABLE_WERROR)
      target_compile_options(${target} PRIVATE /WX)
    endif()
    return()
  endif()

  target_compile_options(${target} PRIVATE
    -Wall
    -Wextra
    -Wpedantic
    -Wshadow
    -Wnon-virtual-dtor
    -Wcast-align
    -Wunused
    -Woverloaded-virtual
    -Wnull-dereference
    -Wdouble-promotion
    -Wformat=2
    -Wimplicit-fallthrough)

  if(AINSC_ENABLE_WERROR)
    target_compile_options(${target} PRIVATE -Werror)
  endif()
endfunction()
