# Shared by our preflight and LadybugDB's cmake-rs build.
if("$ENV{OPENSSL_DIR}" STREQUAL "")
    message(FATAL_ERROR "OPENSSL_DIR must point to the prepared static OpenSSL")
endif()
set(OPENSSL_ROOT_DIR "$ENV{OPENSSL_DIR}" CACHE PATH "Prepared OpenSSL" FORCE)
set(OPENSSL_USE_STATIC_LIBS TRUE CACHE BOOL "No system OpenSSL dependency" FORCE)
set(OPENSSL_INCLUDE_DIR "$ENV{OPENSSL_DIR}/include" CACHE PATH "OpenSSL headers" FORCE)
set(OPENSSL_SSL_LIBRARY "$ENV{OPENSSL_DIR}/lib/libssl.a" CACHE FILEPATH "Static SSL" FORCE)
set(OPENSSL_CRYPTO_LIBRARY "$ENV{OPENSSL_DIR}/lib/libcrypto.a" CACHE FILEPATH "Static crypto" FORCE)
