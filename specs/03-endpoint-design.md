# Endpoint Design

## Overview

The Endpoint design provides a flexible, composable HTTP request handling mechanism. It introduces the `Endpoint` trait for processing HTTP requests and the `HttpService` structure that can be organized in a tree-like hierarchy, allowing requests to be routed and processed through nested service layers.

## Core Concepts

### Endpoint Trait

The `Endpoint` trait defines a contract for routing and handling HTTP requests:

- **Purpose**: Provides a uniform interface for routing and processing HTTP requests
- **Route Method**: `route()` receives a `Request` and a slice of `Address<HttpService>` references, returns an optional reference to an `Address<HttpService>`
- **Handle Leaf Method**: `handle_leaf()` receives a `Request` and returns a `Response` for handling requests locally
- **Routing Decision**: `route()` returns `Some(&Address)` to forward the request to that service, or `None` to handle the request locally via `handle_leaf()`
- **Flexibility**: Allows different implementations to handle requests in various ways
- **Composability**: Endpoints can be composed and chained together

### HttpService Structure

The `HttpService` is a service that wraps an `Endpoint` and can forward requests to multiple `HttpService` instances:

- **Endpoint**: Contains a `dyn Endpoint` trait object for request processing
- **Next Services**: Contains a `Vec<Address<HttpService>>` for forwarding requests to multiple services
- **Tree Structure**: The vector of addresses allows building a tree-like hierarchy of services
- **Routing Logic**: The endpoint can decide which service (if any) to forward the request to based on the request properties

## Design Principles

### 1. Endpoint Trait

The `Endpoint` trait provides both routing and handling capabilities:

```rust
#[async_trait]
pub trait Endpoint: Send + Sync + 'static {
    /// Route the request to one of the next services, or return None to handle locally
    async fn route(
        &self, 
        request: Request, 
        next: &[Address<HttpService>]
    ) -> Option<&Address<HttpService>>;
    
    /// Handle the request locally when route() returns None
    async fn handle_leaf(&self, request: Request) -> Response;
}
```

Key characteristics:
- **Async Processing**: All request handling is asynchronous
- **Route Method**: Receives the HTTP request and a slice of addresses to potential next services
- **Routing Decision**: Returns `Some(&Address)` to forward to that service, or `None` to handle locally
- **Reference Return**: Returns a reference to one of the addresses in the `next` slice
- **Handle Leaf Method**: Processes requests locally when routing returns `None`
- **Separation of Concerns**: Routing logic is separated from request handling logic
- **Generic Usage**: Used as a trait bound in `HttpService<E: Endpoint>` for zero-cost abstraction
- **Trait Object Support**: Can also be used as `dyn Endpoint` when type erasure is needed for heterogeneous service trees

### 2. HttpService Structure

The `HttpService` combines an endpoint with multiple forwarding targets:

```rust
pub struct HttpService<E: Endpoint> {
    endpoint: E,
    next: Vec<Address<HttpService<dyn Endpoint>>>,
    self_addr: Option<Address<HttpService<E>>>,
}
```

Key characteristics:
- **Generic Endpoint**: The endpoint is a generic type parameter `E: Endpoint`, avoiding dynamic dispatch overhead
- **Type Safety**: Compile-time type checking ensures the endpoint implements the `Endpoint` trait
- **Zero-Cost Abstraction**: No runtime overhead from trait objects when using concrete types
- **Next Services**: Vector of addresses to other `HttpService` instances for forwarding
- **Self Address**: Stores the service's own address, obtained from `Context` during `started()` hook
- **Tree Organization**: The vector enables hierarchical service structures with multiple branches
- **Composability**: Services can be nested and composed in various ways
- **Multiple Targets**: Supports forwarding to multiple potential services, with the endpoint deciding which one
- **Type Erasure for Next**: The `next` field uses `Address<HttpService<dyn Endpoint>>` to allow forwarding to services with different endpoint types

### 2.1 Helper Methods

`HttpService` provides convenient helper methods for building and managing service trees:

#### Constructor Methods

```rust
impl<E: Endpoint> HttpService<E> {
    /// Create a new HttpService with the given endpoint and no next services
    pub fn new(endpoint: E) -> Self {
        Self {
            endpoint,
            next: Vec::new(),
            self_addr: None,
        }
    }
    
    /// Create a new HttpService with the given endpoint and initial next services
    pub fn with_next(endpoint: E, next: Vec<Address<HttpService<dyn Endpoint>>>) -> Self {
        Self {
            endpoint,
            next,
            self_addr: None,
        }
    }
}
```

#### Builder Methods

```rust
impl<E: Endpoint> HttpService<E> {
    /// Append a next service address to the service tree
    pub fn append_next(mut self, addr: Address<HttpService<dyn Endpoint>>) -> Self {
        self.next.push(addr);
        self
    }
    
    /// Append multiple next service addresses
    pub fn append_nexts(mut self, addrs: impl IntoIterator<Item = Address<HttpService<dyn Endpoint>>>) -> Self {
        self.next.extend(addrs);
        self
    }
    
    /// Set the next services, replacing any existing ones
    pub fn set_next(mut self, next: Vec<Address<HttpService<dyn Endpoint>>>) -> Self {
        self.next = next;
        self
    }
    
    /// Get a reference to the next services
    pub fn next(&self) -> &[Address<HttpService<dyn Endpoint>>] {
        &self.next
    }
    
    /// Get a mutable reference to the next services
    pub fn next_mut(&mut self) -> &mut Vec<Address<HttpService<dyn Endpoint>>> {
        &mut self.next
    }
    
    /// Get a reference to the endpoint
    pub fn endpoint(&self) -> &E {
        &self.endpoint
    }
    
    /// Get a mutable reference to the endpoint
    pub fn endpoint_mut(&mut self) -> &mut E {
        &mut self.endpoint
    }
    
    /// Get the address of this service
    /// 
    /// This method returns a reference to the service's address, which can be used
    /// for routing and forwarding requests. The address is set during the `started()`
    /// hook when the service starts, using `ctx.addr()` from the `Context`.
    /// 
    /// # Panics
    /// 
    /// This method will panic if called before the service has been started, as
    /// `self_addr` will be `None`. The address is only available after `started()`
    /// has been called.
    pub fn addr(&self) -> &Address<HttpService<E>> {
        self.self_addr.as_ref().expect("Service address not set. Service must be started first.")
    }
}
```

#### Type Conversion Helpers

```rust
impl<E: Endpoint> HttpService<E> {
    /// Convert this service to a boxed trait object for heterogeneous composition
    /// 
    /// Note: Since `HttpService<dyn Endpoint>` cannot be directly constructed
    /// (dyn Endpoint cannot be used as a generic type parameter in this context),
    /// this method would need to be implemented differently. The actual implementation
    /// may involve wrapping the service or using a different approach for type erasure.
    /// 
    /// For now, address conversion via `Into` trait is the primary mechanism
    /// for heterogeneous service composition.
}

// Helper trait for converting Address types
// This allows automatic conversion when adding addresses to next
impl<E: Endpoint> From<Address<HttpService<E>>> for Address<HttpService<dyn Endpoint>>
where
    E: 'static,
{
    fn from(addr: Address<HttpService<E>>) -> Self {
        // Implementation details for address conversion
        // This allows automatic conversion when adding addresses to next
        // The exact mechanism will be determined during implementation
    }
}
```

Key benefits of helper methods:
- **Convenience**: Simplifies service construction and composition
- **Builder Pattern**: Supports fluent API for building service trees
- **Type Safety**: Maintains type safety while providing ergonomic APIs
- **Flexibility**: Allows both immutable and mutable access patterns
- **Composability**: Makes it easy to build complex service hierarchies

### 3. Tree-Like Organization

The design enables tree-like service organization:

- **Root Service**: The outermost `HttpService` receives HTTP requests via its `Address`
- **Inner Services**: Can be nested through the `next` field (vector of addresses)
- **Routing Logic**: Each endpoint can implement custom logic to decide:
  - Whether to process the request itself (return `None`)
  - Which service in `next` to forward to (return `Some(&Address)`)
  - How to route based on request properties (path, headers, method, etc.)
- **Multiple Branches**: The vector allows multiple potential forwarding targets, with the endpoint selecting one

### 4. Request Flow

The request flow through the service tree:

1. HTTP request arrives at the root `HttpService` via `Address<HttpService>`
2. Root service receives the request through its message handler
3. Root service calls `endpoint.route(request, &next)`:
   - Endpoint receives the request and the slice of next service addresses
   - Endpoint returns `Some(&Address)` to forward, or `None` to handle locally
4. If endpoint returns `Some(&Address)`, the request is forwarded to that service
5. If endpoint returns `None`, the service calls `endpoint.handle_leaf(request)` to process the request locally
6. Inner services follow the same pattern recursively
7. Response flows back through the service chain

## Architecture

### Service Implementation

`HttpService` implements the `Service` trait to participate in the actor system:

```rust
#[async_trait]
impl<E: Endpoint> Service for HttpService<E> {
    async fn started(&mut self, ctx: &mut Context<Self>) {
        // Store the service's own address obtained from Context
        self.self_addr = Some(ctx.addr());
        // Additional initialization logic can be added here
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
impl<E: Endpoint> Handler<HttpRequest> for HttpService<E> {
    async fn handle(
        &mut self,
        msg: HttpRequest,
        _ctx: &mut Context<Self>,
    ) -> Response {
        // Call endpoint to decide routing
        if let Some(next_addr) = self.endpoint.route(msg.request, &self.next).await {
            // Forward to the selected service
            next_addr.call(HttpRequest {
                request: msg.request,
            }).await.unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(BoxBody::from("Service unavailable"))
                    .unwrap()
            })
        } else {
            // Endpoint decided to handle locally, call handle_leaf
            self.endpoint.handle_leaf(msg.request).await
        }
    }
}
```

### Tree Structure Example

A tree-like service organization:

```
Root HttpService (handles all requests)
├── endpoint: RouterEndpoint (routes based on path)
└── next: [Address<HttpService>, Address<HttpService>]
    │
    ├── API HttpService (handles /api/*)
    │   ├── endpoint: ApiRouterEndpoint
    │   └── next: [Address<HttpService>]
    │       │
    │       └── Handler HttpService (actual business logic)
    │           ├── endpoint: BusinessLogicEndpoint
    │           └── next: [] (leaf node)
    │
    └── Static HttpService (handles static files)
        ├── endpoint: StaticFileEndpoint
        └── next: [] (leaf node)
```

### Request Processing Patterns

#### Pattern 1: Simple Forwarding

The endpoint forwards all requests to the first next service:

```rust
pub struct ForwardingEndpoint;

#[async_trait]
impl Endpoint for ForwardingEndpoint {
    async fn route(
        &self, 
        _request: Request, 
        next: &[Address<HttpService>]
    ) -> Option<&Address<HttpService>> {
        next.first()
    }
    
    async fn handle_leaf(&self, _request: Request) -> Response {
        // This should never be called if route always returns Some
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::from("No handler available"))
            .unwrap()
    }
}
```

#### Pattern 2: Path-Based Routing

The endpoint routes based on request path:

```rust
pub struct PathRouterEndpoint;

#[async_trait]
impl Endpoint for PathRouterEndpoint {
    async fn route(
        &self, 
        request: Request, 
        next: &[Address<HttpService>]
    ) -> Option<&Address<HttpService>> {
        let path = request.uri().path();
        
        if path.starts_with("/api/") {
            // Forward to first service (API handler)
            next.get(0)
        } else if path.starts_with("/static/") {
            // Forward to second service (static file handler)
            next.get(1)
        } else {
            // Handle locally (return None)
            None
        }
    }
    
    async fn handle_leaf(&self, request: Request) -> Response {
        // Handle requests that don't match any route
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::from(format!("Path not found: {}", request.uri().path())))
            .unwrap()
    }
}
```

#### Pattern 3: Method-Based Routing

The endpoint routes based on HTTP method:

```rust
pub struct MethodRouterEndpoint;

#[async_trait]
impl Endpoint for MethodRouterEndpoint {
    async fn route(
        &self, 
        request: Request, 
        next: &[Address<HttpService>]
    ) -> Option<&Address<HttpService>> {
        match *request.method() {
            Method::GET => next.get(0),      // GET handler
            Method::POST => next.get(1),     // POST handler
            Method::PUT => next.get(2),      // PUT handler
            _ => None,                       // Handle other methods locally
        }
    }
    
    async fn handle_leaf(&self, request: Request) -> Response {
        // Handle unsupported methods
        Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(BoxBody::from(format!("Method {} not allowed", request.method())))
            .unwrap()
    }
}
```

## Design Decisions

### Why Vec<Address<HttpService>>?

Using `Vec<Address<HttpService>>` enables flexible service composition:

- **Multiple Targets**: Services can forward to multiple potential targets
- **Routing Flexibility**: Endpoint can choose which service to forward to based on request properties
- **Tree Structure**: Enables hierarchical organization of services with multiple branches
- **Type Safety**: The addresses are type-safe and ensure only `HttpService` instances can be chained
- **Dynamic Selection**: Endpoint can dynamically select which service to forward to at runtime

### Why Return Option<&Address<HttpService>>?

Returning `Option<&Address<HttpService>>` from `route` provides clear routing semantics:

- **Explicit Routing**: Endpoint explicitly decides whether to forward (Some) or handle locally (None)
- **Reference Safety**: Returns a reference to one of the addresses in the input slice, ensuring lifetime safety
- **Clear Contract**: Makes the routing decision explicit and easy to understand
- **Flexibility**: Allows endpoints to implement various routing strategies

### Why Separate route() and handle_leaf()?

Separating routing and handling into two methods provides several benefits:

- **Separation of Concerns**: Routing logic is separate from request processing logic
- **Clear Semantics**: When `route()` returns `None`, it's clear that `handle_leaf()` will be called
- **Flexibility**: Endpoints can implement complex routing without mixing it with handling logic
- **Testability**: Routing and handling can be tested independently
- **Composability**: Different routing strategies can be combined with different handling strategies

### Why Generic Endpoint Instead of dyn Endpoint?

Using a generic type parameter `E: Endpoint` instead of `Box<dyn Endpoint>` provides several advantages:

- **Zero-Cost Abstraction**: No runtime overhead from dynamic dispatch when using concrete types
- **Compile-Time Optimization**: The compiler can inline and optimize endpoint methods
- **Type Safety**: Stronger type checking at compile time
- **Performance**: Direct method calls instead of virtual function calls through trait objects
- **Flexibility**: Still allows using trait objects in `next` field for heterogeneous service trees
- **Composability**: Endpoints can be composed at compile time with full type information

However, the `next` field uses `Address<HttpService<dyn Endpoint>>` to allow forwarding to services with different endpoint types, providing runtime flexibility for service composition.

### Type Conversion for Heterogeneous Services

When building service trees with different endpoint types, services need to be converted to `HttpService<dyn Endpoint>`:

- **Type Erasure**: Concrete `HttpService<E>` instances can be converted to `HttpService<dyn Endpoint>` for storage in the `next` vector
- **Runtime Flexibility**: Allows composing services with different endpoint implementations at runtime
- **Performance Trade-off**: The conversion to trait objects introduces dynamic dispatch for the endpoint methods, but only for services in the `next` vector
- **Homogeneous Trees**: If all services use the same endpoint type, no conversion is needed and full compile-time optimization is preserved

The exact mechanism for type conversion (e.g., `Into` trait, helper methods, or explicit casting) will be determined during implementation.

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
    async fn route(
        &self, 
        _request: Request, 
        _next: &[Address<HttpService>]
    ) -> Option<&Address<HttpService>> {
        // Handle locally, don't forward
        None
    }
    
    async fn handle_leaf(&self, _request: Request) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .body(BoxBody::from("Hello, World!"))
            .unwrap()
    }
}
```

### Creating an HttpService

Using the constructor:

```rust
let endpoint = HelloEndpoint;
let service = HttpService::new(endpoint);

let (addr, future) = service.start();
tokio::spawn(future);
```

Or using the builder pattern:

```rust
let service = HttpService::new(HelloEndpoint)
    .append_next(api_addr)
    .append_next(static_addr);

let (addr, future) = service.start();
tokio::spawn(future);
```

### Chaining Services

When chaining services with different endpoint types, use helper methods for convenient composition:

```rust
// Create API service
let api_service = HttpService::new(ApiEndpoint);
let (api_addr, api_future) = api_service.start();
tokio::spawn(api_future);

// Create static file service
let static_service = HttpService::new(StaticFileEndpoint);
let (static_addr, static_future) = static_service.start();
tokio::spawn(static_future);

// Create router service with multiple next services using builder pattern
let router_service = HttpService::new(PathRouterEndpoint)
    .append_next(api_addr.into())      // Convert to Address<HttpService<dyn Endpoint>>
    .append_next(static_addr.into()); // Convert to Address<HttpService<dyn Endpoint>>

let (router_addr, router_future) = router_service.start();
tokio::spawn(router_future);
```

Alternatively, using `with_next` for initial setup:

```rust
let router_service = HttpService::with_next(
    PathRouterEndpoint,
    vec![
        api_addr.into(),
        static_addr.into(),
    ]
);
let (router_addr, router_future) = router_service.start();
tokio::spawn(router_future);
```

If all services use the same endpoint type, no conversion is needed:

```rust
// All services use the same endpoint type
let api_service = HttpService::new(PathRouterEndpoint);
let (api_addr, api_future) = api_service.start();
tokio::spawn(api_future);

let static_service = HttpService::new(PathRouterEndpoint);
let (static_addr, static_future) = static_service.start();
tokio::spawn(static_future);

// Same type, no conversion needed - can use addresses directly
let router_service = HttpService::new(PathRouterEndpoint)
    .append_next(api_addr)
    .append_next(static_addr);

let (router_addr, router_future) = router_service.start();
tokio::spawn(router_future);
```

### Building Complex Service Trees

Helper methods make it easy to build complex service hierarchies:

```rust
// Build a service tree with multiple layers
let leaf1 = HttpService::new(HelloEndpoint);
let (leaf1_addr, leaf1_future) = leaf1.start();
tokio::spawn(leaf1_future);

let leaf2 = HttpService::new(GoodbyeEndpoint);
let (leaf2_addr, leaf2_future) = leaf2.start();
tokio::spawn(leaf2_future);

// Middle layer routes to different leaves
let middle = HttpService::new(PathRouterEndpoint)
    .append_next(leaf1_addr.into())
    .append_next(leaf2_addr.into());
let (middle_addr, middle_future) = middle.start();
tokio::spawn(middle_future);

// Root layer
let root = HttpService::new(AuthMiddlewareEndpoint)
    .append_next(middle_addr.into());
let (root_addr, root_future) = root.start();
tokio::spawn(root_future);
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

- **Service Discovery**: Mechanisms for discovering and connecting services dynamically
- **Load Balancing**: Support for forwarding to multiple services with load balancing (endpoint could select based on load)
- **Circuit Breaker**: Built-in support for circuit breaker patterns
- **Metrics**: Integration with metrics and observability systems
- **Configuration**: Ways to configure service trees declaratively
- **Error Handling**: Enhanced error handling and error response generation
- **Request/Response Transformation**: Built-in support for transforming requests and responses between layers
- **Multiple Forwarding**: Consider supporting forwarding to multiple services simultaneously (fan-out pattern)
- **Middleware Pattern**: Consider adding middleware support that can process requests/responses before and after forwarding
