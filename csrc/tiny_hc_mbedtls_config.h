/*
 * mbedTLS (TLS/X.509 part) configuration for tiny-hc-tls: a TLS 1.2/1.3
 * client that never verifies certificates. Anything a health check does not
 * need is left out to keep the binary small.
 */
#define MBEDTLS_CONFIG_VERSION 0x04000000

#define MBEDTLS_SSL_TLS_C
#define MBEDTLS_SSL_CLI_C
#define MBEDTLS_SSL_PROTO_TLS1_2
#define MBEDTLS_SSL_PROTO_TLS1_3
#define MBEDTLS_SSL_SERVER_NAME_INDICATION
#define MBEDTLS_SSL_EXTENDED_MASTER_SECRET
#define MBEDTLS_SSL_TLS1_3_COMPATIBILITY_MODE

/* TLS 1.2: ECDHE with RSA or ECDSA certificates. */
#define MBEDTLS_KEY_EXCHANGE_ECDHE_RSA_ENABLED
#define MBEDTLS_KEY_EXCHANGE_ECDHE_ECDSA_ENABLED
/* TLS 1.3: (EC)DHE only, no PSK / session resumption. */
#define MBEDTLS_SSL_TLS1_3_KEY_EXCHANGE_MODE_EPHEMERAL_ENABLED

/* The server certificate is parsed (for its public key) but never verified. */
#define MBEDTLS_X509_USE_C
#define MBEDTLS_X509_CRT_PARSE_C
/* Also gates offering rsa_pss_rsae_* signature algorithms, which TLS 1.3
 * requires for servers with RSA certificates. */
#define MBEDTLS_X509_RSASSA_PSS_SUPPORT
/* Required by the TLS 1.3 implementation. */
#define MBEDTLS_SSL_KEEP_PEER_CERTIFICATE
