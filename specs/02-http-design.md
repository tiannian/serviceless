# Serviceless HTTP Design

## Overview

Serviceless HTTP is the core of an HTTP framework. It provides fundamental types and abstractions for building HTTP-based applications. The module wraps the standard `http` crate's `Request` and `Response` types into its own domain-specific types, providing a foundation for HTTP handling within the serviceless actor model.

## Core Concepts

### HTTP Request Wrapper

The HTTP framework wraps the standard `http::Request` type into its own `Request` type:

- **Purpose**: Provides a domain-specific abstraction over the standard HTTP request
- **Integration**: Seamlessly integrates with the serviceless actor model
- **Extensibility**: Allows for framework-specific extensions and modifications
- **Type Safety**: Maintains type safety while providing additional functionality

### HTTP Response Wrapper

Similarly, the framework wraps `http::Response` into its own `Response` type:

- **Purpose**: Provides a domain-specific abstraction over the standard HTTP response
- **Builder Pattern**: May provide convenient builder methods for constructing responses
- **Integration**: Works seamlessly with the actor message passing system
- **Type Safety**: Ensures type-safe response handling

## Design Principles

### 1. Wrapper Pattern

The framework uses a wrapper pattern to encapsulate standard HTTP types:

- **Encapsulation**: Wraps `http::Request` and `http::Response` without exposing implementation details
- **Abstraction**: Provides a clean abstraction layer over the underlying HTTP types
- **Compatibility**: Maintains compatibility with the standard `http` crate while adding framework-specific features

### 2. Foundation Types

Serviceless HTTP provides fundamental types that serve as building blocks:

- **Core Types**: Request and Response wrappers form the foundation
- **Extensibility**: These types are designed to be extended for specific use cases
- **Reusability**: Common HTTP operations are abstracted into reusable types

### 3. Actor Model Integration

The HTTP types are designed to work within the serviceless actor model:

- **Message Passing**: HTTP requests and responses can be passed as messages between actors
- **Async Handling**: All HTTP operations are asynchronous and compatible with the actor system
- **Type Safety**: Leverages Rust's type system for safe HTTP handling

### 4. No-Std Support

Like other serviceless modules, HTTP support should consider `no_std` environments:

- **Core Requirement**: Core HTTP types should work in `no_std` environments where possible
- **Feature Gating**: Standard library features should be optional and gated behind feature flags
- **Alloc Dependency**: Use the `alloc` crate for heap-allocated types when needed

## Architecture

### Request Flow

1. Incoming HTTP request arrives as `http::Request`
2. Request is wrapped into framework's `Request` type
3. Request can be passed as a message to an actor service
4. Actor processes the request and generates a response
5. Response is wrapped into framework's `Response` type
6. Response is converted back to `http::Response` for transmission

### Response Flow

1. Actor generates a response using framework's `Response` type
2. Response may include status code, headers, and body
3. Response is converted to `http::Response`
4. Response is sent back to the client

## Type Design

### Request Type

The `Request` type wraps `http::Request`:

```rust
pub struct Request<B> {
    inner: http::Request<B>,
    // Framework-specific extensions
}
```

Key characteristics:
- **Generic Body Type**: Supports different body types (e.g., `Vec<u8>`, `String`, custom types)
- **Access Methods**: Provides methods to access underlying request parts
- **Extensions**: May include framework-specific extensions (e.g., route parameters, query parameters)

### Response Type

The `Response` type wraps `http::Response`:

```rust
pub struct Response<B> {
    inner: http::Response<B>,
    // Framework-specific extensions
}
```

Key characteristics:
- **Generic Body Type**: Supports different body types
- **Builder Methods**: May provide convenient methods for constructing responses
- **Status Codes**: Easy access to HTTP status codes
- **Headers**: Convenient header manipulation

## Integration with Actor Model

### Request as Message

HTTP requests can be treated as messages in the actor system:

- **Message Trait**: Request types can implement the `Message` trait
- **Handler Implementation**: Services can implement `Handler<Request>` to process HTTP requests
- **Response Result**: The message result type would be `Response`

### Example Flow

```rust
// Request message type
pub struct HttpRequest {
    request: Request<Body>,
}

impl Message for HttpRequest {
    type Result = Response<Body>;
}

// Service handling HTTP requests
#[async_trait]
impl Handler<HttpRequest> for HttpService {
    async fn handle(
        &mut self, 
        msg: HttpRequest, 
        ctx: &mut Context<Self>
    ) -> Response<Body> {
        // Process request and generate response
        Response::new(Body::from("Hello, World!"))
    }
}
```

## Design Decisions

### Why Wrap Instead of Extend?

The framework wraps `http::Request` and `http::Response` rather than extending them:

- **Control**: Full control over the API surface
- **Extensibility**: Can add framework-specific features without modifying standard types
- **Compatibility**: Maintains compatibility with the standard `http` crate
- **Abstraction**: Provides a clean abstraction layer

### Foundation Types Only

Serviceless HTTP focuses on providing foundation types:

- **Core Responsibility**: Provides only the essential types needed for HTTP handling
- **Framework Building**: Higher-level features (routing, middleware, etc.) are built on top
- **Modularity**: Keeps the core module focused and lightweight

## Future Considerations

- **Body Types**: Support for streaming bodies and different body representations
- **Extensions**: Framework-specific extensions (route params, query params, etc.)
- **Error Handling**: HTTP-specific error types and error responses
- **Middleware**: Support for middleware chains (may be in a separate module)
- **Routing**: URL routing capabilities (may be in a separate module)
- **Serialization**: Integration with serialization libraries for request/response bodies
