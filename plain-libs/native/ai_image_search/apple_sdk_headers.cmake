function(plain_use_apple_sdk_headers)
    if(APPLE AND TARGET Iconv::Iconv)
        get_target_property(sdk_includes Iconv::Iconv INTERFACE_INCLUDE_DIRECTORIES)
        list(FILTER sdk_includes EXCLUDE REGEX "\\.sdk/usr/include/?$")
        set_property(TARGET Iconv::Iconv PROPERTY INTERFACE_INCLUDE_DIRECTORIES "${sdk_includes}")
    endif()
endfunction()

cmake_language(DEFER CALL plain_use_apple_sdk_headers)
