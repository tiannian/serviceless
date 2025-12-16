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
- **Trait Object**: Can be used as `dyn Endpoint` for dynamic dispatch

### 2. HttpService Structure

The `HttpService` combines an endpoint with multiple forwarding targets:

```rust
pub struct HttpService {
    endpoint: Box<dyn Endpoint>,
    next: Vec<Address<HttpService>>,
}
```

Key characteristics:
- **Endpoint**: The primary request handler and router
- **Next Services**: Vector of addresses to other `HttpService` instances for forwarding
- **Tree Organization**: The vector enables hierarchical service structures with multiple branches
- **Composability**: Services can be nested and composed in various ways
- **Multiple Targets**: Supports forwarding to multiple potential services, with the endpoint deciding which one

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

```rust
let endpoint = Box::new(HelloEndpoint);
let service = HttpService {
    endpoint,
    next: Vec::new(), // No next services
};

let (addr, future) = service.start();
tokio::spawn(future);
```

### Chaining Services

```rust
// Create API service
let api_endpoint = Box::new(ApiEndpoint);
let api_service = HttpService {
    endpoint: api_endpoint,
    next: Vec::new(),
};
let (api_addr, api_future) = api_service.start();
tokio::spawn(api_future);

// Create static file service
let static_endpoint = Box::new(StaticFileEndpoint);
let static_service = HttpService {
    endpoint: static_endpoint,
    next: Vec::new(),
};
let (static_addr, static_future) = static_service.start();
tokio::spawn(static_future);

// Create router service with multiple next services
let router_endpoint = Box::new(PathRouterEndpoint);
let router_service = HttpService {
    endpoint: router_endpoint,
    next: vec![api_addr, static_addr], // Multiple forwarding targets
};
let (router_addr, router_future) = router_service.start();
tokio::spawn(router_future);
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
