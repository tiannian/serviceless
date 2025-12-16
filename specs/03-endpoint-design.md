# Endpoint Design

## Overview

The Endpoint design provides a flexible, composable HTTP request handling mechanism. It introduces the `Endpoint` trait for processing HTTP requests and the `HttpService` structure that can be organized in a tree-like hierarchy, allowing requests to be routed and processed through nested service layers.

## Core Concepts

### Endpoint Trait

The `Endpoint` trait defines a contract for handling HTTP requests:

- **Purpose**: Provides a uniform interface for processing HTTP requests
- **Method**: `handle_http()` receives a `Request` and returns a `Response`
- **Flexibility**: Allows different implementations to handle requests in various ways
- **Composability**: Endpoints can be composed and chained together

### HttpService Structure

The `HttpService` is a service that wraps an `Endpoint` and can optionally forward requests to another `HttpService`:

- **Endpoint**: Contains a `dyn Endpoint` trait object for request processing
- **Next Service**: Contains an `Option<Address<HttpService>>` for forwarding requests
- **Tree Structure**: The optional address allows building a tree-like hierarchy of services
- **Routing Logic**: The outer service can implement custom logic to decide whether and how to forward requests to inner services

## Design Principles

### 1. Endpoint Trait

The `Endpoint` trait provides a simple, focused interface:

```rust
#[async_trait]
pub trait Endpoint: Send + Sync + 'static {
    async fn handle_http(&self, request: Request) -> Response;
}
```

Key characteristics:
- **Async Processing**: All request handling is asynchronous
- **Ownership**: Takes ownership of the request
- **Simple Contract**: Single method with clear input/output types
- **Trait Object**: Can be used as `dyn Endpoint` for dynamic dispatch

### 2. HttpService Structure

The `HttpService` combines an endpoint with optional forwarding capability:

```rust
pub struct HttpService {
    endpoint: Box<dyn Endpoint>,
    next: Option<Address<HttpService>>,
}
```

Key characteristics:
- **Endpoint**: The primary request handler
- **Next Service**: Optional address to another `HttpService` for forwarding
- **Tree Organization**: The optional address enables hierarchical service structures
- **Composability**: Services can be nested and composed in various ways

### 3. Tree-Like Organization

The design enables tree-like service organization:

- **Root Service**: The outermost `HttpService` receives HTTP requests via its `Address`
- **Inner Services**: Can be nested through the `next` field
- **Routing Logic**: Each service can implement custom logic to decide:
  - Whether to process the request itself
  - Whether to forward to the next service
  - How to modify the request before forwarding
  - How to combine responses from multiple services

### 4. Request Flow

The request flow through the service tree:

1. HTTP request arrives at the root `HttpService` via `Address<HttpService>`
2. Root service receives the request through its message handler
3. Root service can:
   - Process the request using its `endpoint`
   - Forward the request to `next` service (if present)
   - Combine both approaches (e.g., middleware pattern)
4. Inner services follow the same pattern recursively
5. Response flows back through the service chain

## Architecture

### Service Implementation

`HttpService` implements the `Service` trait to participate in the actor system:

```rust
#[async_trait]
impl Service for HttpService {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        // Initialization logic
    }
}

// HttpService handles HTTP requests as messages
pub struct HttpRequest {
    request: Request,
}

impl Message for HttpRequest {
    type Result = Response;
}

#[async_trait]
impl Handler<HttpRequest> for HttpService {
    async fn handle(
        &mut self,
        msg: HttpRequest,
        _ctx: &mut Context<Self>,
    ) -> Response {
        // Process request through endpoint
        let response = self.endpoint.handle_http(msg.request).await;
        
        // Optionally forward to next service
        if let Some(ref next_addr) = self.next {
            // Custom routing logic here
            // For example, forward based on path, headers, etc.
            let forwarded_response = next_addr.call(HttpRequest {
                request: msg.request, // or modified request
            }).await?;
            
            // Combine or replace response
            return forwarded_response;
        }
        
        response
    }
}
```

### Tree Structure Example

A tree-like service organization:

```
Root HttpService (handles all requests)
├── endpoint: RouterEndpoint (routes based on path)
└── next: Some(Address<HttpService>)
    │
    └── Middleware HttpService (adds logging)
        ├── endpoint: LoggingEndpoint
        └── next: Some(Address<HttpService>)
            │
            └── Handler HttpService (actual business logic)
                ├── endpoint: BusinessLogicEndpoint
                └── next: None (leaf node)
```

### Request Processing Patterns

#### Pattern 1: Simple Forwarding

The service forwards all requests to the next service:

```rust
async fn handle(&mut self, msg: HttpRequest, _ctx: &mut Context<Self>) -> Response {
    if let Some(ref next_addr) = self.next {
        next_addr.call(msg).await.unwrap_or_else(|_| {
            Response::new(BoxBody::from("Service unavailable"))
        })
    } else {
        self.endpoint.handle_http(msg.request).await
    }
}
```

#### Pattern 2: Middleware Pattern

The service processes the request, then forwards, then processes the response:

```rust
async fn handle(&mut self, msg: HttpRequest, _ctx: &mut Context<Self>) -> Response {
    // Pre-processing
    let request = self.endpoint.handle_http(msg.request).await;
    
    // Forward to next service
    if let Some(ref next_addr) = self.next {
        let response = next_addr.call(HttpRequest { request }).await?;
        // Post-processing on response
        return response;
    }
    
    request
}
```

#### Pattern 3: Conditional Routing

The service routes based on request properties:

```rust
async fn handle(&mut self, msg: HttpRequest, _ctx: &mut Context<Self>) -> Response {
    let path = msg.request.uri().path();
    
    if path.starts_with("/api/") {
        // Forward to API service
        if let Some(ref api_addr) = self.next {
            return api_addr.call(msg).await?;
        }
    }
    
    // Handle with own endpoint
    self.endpoint.handle_http(msg.request).await
}
```

## Design Decisions

### Why Option<Address<HttpService>>?

Using `Option<Address<HttpService>>` enables flexible service composition:

- **Optional Forwarding**: Services can be leaf nodes (None) or intermediate nodes (Some)
- **Tree Structure**: Enables hierarchical organization of services
- **Type Safety**: The address is type-safe and ensures only `HttpService` instances can be chained
- **Flexibility**: Services can dynamically decide whether to forward based on runtime conditions

### Why dyn Endpoint?

Using `dyn Endpoint` provides flexibility:

- **Dynamic Dispatch**: Different endpoint implementations can be used without changing the service structure
- **Composability**: Endpoints can be swapped and composed easily
- **Simplicity**: Avoids complex generic type parameters
- **Runtime Flexibility**: Endpoint behavior can be determined at runtime

### Tree Structure Benefits

The tree-like organization provides several advantages:

- **Modularity**: Each service layer can focus on a specific concern
- **Reusability**: Services can be reused in different tree configurations
- **Testability**: Each service can be tested independently
- **Flexibility**: Easy to add, remove, or reorder service layers
- **Middleware Pattern**: Natural support for middleware-like patterns

## Examples

### Basic Endpoint Implementation

```rust
pub struct HelloEndpoint;

#[async_trait]
impl Endpoint for HelloEndpoint {
    async fn handle_http(&self, request: Request) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .body(BoxBody::from("Hello, World!"))
            .unwrap()
    }
}
```

### Creating an HttpService

```rust
let endpoint = Box::new(HelloEndpoint);
let service = HttpService {
    endpoint,
    next: None,
};

let (addr, future) = service.start();
tokio::spawn(future);
```

### Chaining Services

```rust
// Create inner service
let inner_endpoint = Box::new(ApiEndpoint);
let inner_service = HttpService {
    endpoint: inner_endpoint,
    next: None,
};
let (inner_addr, inner_future) = inner_service.start();
tokio::spawn(inner_future);

// Create outer service with forwarding
let outer_endpoint = Box::new(RouterEndpoint);
let outer_service = HttpService {
    endpoint: outer_endpoint,
    next: Some(inner_addr),
};
let (outer_addr, outer_future) = outer_service.start();
tokio::spawn(outer_future);
```

### Using the Service

```rust
let request = Request::builder()
    .uri("/api/users")
    .body(BoxBody::empty())
    .unwrap();

let response = outer_addr.call(HttpRequest { request }).await?;
```

## Integration with Actor Model

### Service as Actor

`HttpService` participates in the actor system:

- **Service Trait**: Implements `Service` trait to run as an actor
- **Message Handling**: Handles `HttpRequest` messages through `Handler` trait
- **Address**: Can be addressed via `Address<HttpService>`
- **Async Processing**: All operations are asynchronous and non-blocking

### Request as Message

HTTP requests are wrapped in message types:

- **Message Type**: `HttpRequest` wraps the `Request` type
- **Result Type**: `Response` is the message result type
- **Type Safety**: Leverages Rust's type system for safe message handling

## Future Considerations

- **Multiple Next Services**: Consider supporting multiple next services for more complex routing
- **Service Discovery**: Mechanisms for discovering and connecting services dynamically
- **Load Balancing**: Support for forwarding to multiple services with load balancing
- **Circuit Breaker**: Built-in support for circuit breaker patterns
- **Metrics**: Integration with metrics and observability systems
- **Configuration**: Ways to configure service trees declaratively
- **Error Handling**: Enhanced error handling and error response generation
- **Request/Response Transformation**: Built-in support for transforming requests and responses between layers
