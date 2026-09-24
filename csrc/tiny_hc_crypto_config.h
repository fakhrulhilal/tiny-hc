/*
 * TF-PSA-Crypto configuration for tiny-hc-tls: just enough crypto for the
 * cipher suites and key exchanges enabled in mbedtls_config.h.
 */
#define TF_PSA_CRYPTO_CONFIG_VERSION 0x01000000

/* Record protection: AES-GCM (mandatory in TLS 1.3, ubiquitous in TLS 1.2). */
#define PSA_WANT_KEY_TYPE_AES
#define PSA_WANT_ALG_GCM
#define MBEDTLS_AES_FEWER_TABLES

/* Hashes, HMAC and key derivation for both protocol versions. */
#define PSA_WANT_ALG_SHA_256
#define PSA_WANT_ALG_SHA_384
#define PSA_WANT_ALG_SHA_512
#define PSA_WANT_ALG_HMAC
#define PSA_WANT_KEY_TYPE_HMAC
#define PSA_WANT_ALG_HKDF
#define PSA_WANT_ALG_HKDF_EXTRACT
#define PSA_WANT_ALG_HKDF_EXPAND
#define PSA_WANT_ALG_TLS12_PRF
#define PSA_WANT_KEY_TYPE_DERIVE
#define PSA_WANT_KEY_TYPE_RAW_DATA

/* Ephemeral key exchange: X25519, P-256, P-384. */
#define PSA_WANT_ALG_ECDH
#define PSA_WANT_ECC_MONTGOMERY_255
#define PSA_WANT_ECC_SECP_R1_256
#define PSA_WANT_ECC_SECP_R1_384
#define PSA_WANT_KEY_TYPE_ECC_PUBLIC_KEY
#define PSA_WANT_KEY_TYPE_ECC_KEY_PAIR_BASIC
#define PSA_WANT_KEY_TYPE_ECC_KEY_PAIR_IMPORT
#define PSA_WANT_KEY_TYPE_ECC_KEY_PAIR_EXPORT
#define PSA_WANT_KEY_TYPE_ECC_KEY_PAIR_GENERATE

/* Server handshake signatures (checked even though the certificate is not). */
#define PSA_WANT_ALG_ECDSA
#define PSA_WANT_ALG_RSA_PSS
#define PSA_WANT_ALG_RSA_PKCS1V15_SIGN
#define PSA_WANT_KEY_TYPE_RSA_PUBLIC_KEY
/* Required by MBEDTLS_KEY_EXCHANGE_ECDHE_RSA_ENABLED; unused code is dropped by the linker. */
#define PSA_WANT_KEY_TYPE_RSA_KEY_PAIR_BASIC

#define MBEDTLS_PSA_CRYPTO_C
#define MBEDTLS_PSA_BUILTIN_GET_ENTROPY
#define MBEDTLS_PSA_KEY_STORE_DYNAMIC
#define MBEDTLS_CTR_DRBG_C
#define MBEDTLS_MD_C
#define MBEDTLS_PK_C
#define MBEDTLS_PK_PARSE_C
#define MBEDTLS_ASN1_PARSE_C
