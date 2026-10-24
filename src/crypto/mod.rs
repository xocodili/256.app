//! Cryptography policy and primitives for this API.
//!
//! # Transport (TLS)
//! Terminate TLS at the edge (reverse proxy or cloud LB) with **TLS 1.3** and prefer **hybrid**
//! key exchange (classical + post-quantum KEM, e.g. X25519 + ML-KEM) where your platform supports
//! it. The Rust app typically sees plaintext HTTP behind the proxy; PQC is enforced at the edge.
//!
//! # Symmetric data at rest
//! Use **AES-256-GCM** or **ChaCha20-Poly1305** for field- or blob-level encryption. Grover's
//! algorithm weakens symmetric keys roughly quadratically; **256-bit keys** remain the conservative
//! choice for a post-quantum horizon.
//!
//! # Cryptographic hashing — **SHA3-512** (standard)
//! **SHA3-512** is the default **one-way hash** for fingerprints, stored verification tokens,
//! integrity checks, and similar uses ([`sha3`] module). Hashing is **not encryption** (there is
//! no secret key to “decrypt” a digest).
//!
//! # Passwords — **Argon2id** (not SHA3-512)
//! Passwords use **Argon2id** with a modern memory cost (e.g. 19–64 MiB), time ≥ 3, and
//! parallelism tuned to your CPU budget. Fast hashes like SHA3-512 are **unsuitable** for
//! password storage (they are too fast to brute-force). Quantum computers do not replace the need
//! for memory-hard password hashing.
//!
//! # Public-key signatures (tokens, webhooks, inter-service)
//! Plan a **hybrid** transition: keep Ed25519/ECDSA today while adding **ML-DSA** (FIPS 204) or
//! **SLH-DSA** where standards and library support match your compliance needs.

pub mod sha3;
