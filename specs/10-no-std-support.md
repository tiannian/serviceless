# No-Std Support Specification

## Overview

This codebase aims to support `no_std` environments where the Rust standard library is not available. This document specifies the requirements, patterns, and guidelines for implementing and maintaining no_std compatibility across all modules.

## Current Status

### Channel Module

The `service-channel` crate currently supports no_std:

- Uses `#![no_std]` attribute
- Relies on `alloc` crate for heap allocations
- Provides optional `std` feature for enhanced functionality
- Uses feature gates to conditionally enable std-specific code

### Actor Module

The `serviceless-actor` crate currently requires std:

- Uses `std::future::Future` and other std types
- Depends on std-only dependencies (e.g., `log`, `thiserror`)
- Needs refactoring to support no_std environments

## Requirements

### Core Principles

1. **Default no_std**: All crates should compile with `#![no_std]` by default
2. **Optional std feature**: Standard library features should be gated behind a `std` feature flag
3. **Alloc dependency**: Use `alloc` crate for heap-allocated types (Vec, String, Box, etc.)
4. **No unsafe code**: Maintain the current policy of avoiding unsafe code where possible

### Feature Flag Pattern

All crates should follow this feature flag pattern:

```toml
[features]
default = ["std"]
std = []
```

- `default` feature includes `std` for convenience
- Users can opt out by using `default-features = false`
- `std` feature enables std-specific functionality

### Code Organization

#### Library Root

```rust
#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;
```

#### Conditional Imports

Use conditional compilation for std-specific imports:

```rust
#[cfg(feature = "std")]
use std::error::Error;

#[cfg(not(feature = "std"))]
use core::error::Error;
```

#### Type Aliases

Use type aliases to abstract over std/core differences:

```rust
#[cfg(feature = "std")]
use std::future::Future;

#[cfg(not(feature = "std"))]
use core::future::Future;
```

## Implementation Guidelines

### Dependencies

#### Allowed Dependencies

- `alloc`: For heap-allocated collections and types
- `core`: Automatically available in no_std (no need to declare)
- Async runtime-agnostic crates that support no_std

#### Conditional Dependencies

Dependencies that require std should be feature-gated:

```toml
[dependencies]
some-crate = { version = "1.0", optional = true }

[features]
std = ["some-crate"]
```

#### Workspace Dependencies

Workspace dependencies should be evaluated for no_std compatibility:

- `futures-core`, `futures-sink`: Support no_std
- `async-trait`: May require std (needs verification)
- `log`: Requires std (should be feature-gated)
- `thiserror`: Requires std (should be feature-gated or replaced)

### Error Handling

#### Current Approach

The actor module uses `thiserror` which requires std. For no_std support:

**Option 1**: Feature-gate error types:
```rust
#[cfg(feature = "std")]
use thiserror::Error;

#[cfg(not(feature = "std"))]
// Use manual error implementation
```

**Option 2**: Use `defmt` or similar no_std-compatible error handling

**Option 3**: Implement custom error types without derive macros

### Logging

The `log` crate requires std. For no_std support:

**Option 1**: Feature-gate logging:
```rust
#[cfg(feature = "std")]
log::info!("message");

#[cfg(not(feature = "std"))]
// Use defmt, embedded-log, or no logging
```

**Option 2**: Use no_std-compatible logging crates:
- `defmt`: For embedded systems
- `embedded-log`: For embedded logging
- `ufmt`: Minimal formatting

### Future Types

Replace `std::future::Future` with `core::future::Future`:

```rust
// Before
use std::future::Future;

// After
#[cfg(feature = "std")]
use std::future::Future;

#[cfg(not(feature = "std"))]
use core::future::Future;
```

### Collections

Use `alloc` collections instead of `std`:

```rust
use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use alloc::sync::Arc;
```

### Async Runtime

The actor system should remain runtime-agnostic:

- Do not depend on specific async runtimes (tokio, async-std, etc.)
- Runtime dependencies should be in examples and tests only
- Use `core::future::Future` for all async types

## Testing

### Test Organization

- Unit tests should work in both std and no_std contexts when possible
- Integration tests can use std features
- Use feature flags to conditionally compile test code

### Example Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[cfg(feature = "std")]
    #[test]
    fn test_with_std() {
        // std-specific tests
    }
    
    #[test]
    fn test_no_std() {
        // no_std compatible tests
    }
}
```

## Migration Strategy

### Phase 1: Channel Module (Complete)

- ✅ Already supports no_std
- ✅ Uses feature gates correctly
- ✅ Relies on alloc crate

### Phase 2: Actor Module (Pending)

1. Replace `std::future::Future` with conditional imports
2. Feature-gate `log` usage
3. Replace or feature-gate `thiserror`
4. Replace `std` collections with `alloc` equivalents
5. Add `#![no_std]` attribute
6. Update Cargo.toml with feature flags

### Phase 3: Verification

1. Test compilation with `--no-default-features`
2. Verify functionality in no_std environment
3. Update documentation
4. Add CI checks for no_std builds

## Compatibility Matrix

| Component | Current Status | Target Status |
|-----------|---------------|---------------|
| `service-channel` | ✅ no_std ready | ✅ Maintain |
| `serviceless-actor` | ❌ Requires std | ✅ no_std ready |
| Error types | ⚠️ Uses thiserror | ✅ Feature-gated or replaced |
| Logging | ⚠️ Uses log crate | ✅ Feature-gated or replaced |
| Future types | ⚠️ Uses std::future | ✅ Uses core::future |
| Collections | ⚠️ Uses std | ✅ Uses alloc |

## Examples

### Minimal no_std Crate

```rust
#![no_std]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

use alloc::vec::Vec;

pub fn example() -> Vec<u8> {
    Vec::new()
}
```

### Feature-Gated Functionality

```rust
#[cfg(feature = "std")]
pub fn std_only_function() {
    // std-specific code
}

#[cfg(not(feature = "std"))]
pub fn no_std_alternative() {
    // no_std alternative
}
```

## References

- [Rust Embedded Book - no_std](https://docs.rust-embedded.org/book/intro/no-std.html)
- [The Embedded Rust Book](https://docs.rust-embedded.org/book/)
- [no_std crates.io guide](https://github.com/rust-lang/api-guidelines/blob/master/src/interoperability.md#c-no-std)
