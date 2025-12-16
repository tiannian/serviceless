# Method Endpoint

## Overview

This document describes the implementation of `MethodEndpoint`, a specialized endpoint implementation that provides HTTP method filtering capabilities. The `MethodEndpoint` acts as a middleware-like filter that checks the HTTP method of incoming requests and either forwards matching requests to downstream services or returns error responses for non-matching requests.

## Core Concepts

### MethodEndpoint

The `MethodEndpoint` filters requests based on HTTP method:

- **Purpose**: Ensures only specific HTTP methods are allowed to pass through
- **Multiple Methods Support**: Supports filtering for one or multiple HTTP methods simultaneously
- **Filtering Logic**: Checks if the request method is in the set of allowed methods
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Handling**: Returns `405 Method Not Allowed` with `Allow` header listing all allowed methods for non-matching requests

## Design Principles

### MethodEndpoint Implementation

The `MethodEndpoint` filters requests by HTTP method, supporting multiple allowed methods:

```rust
use http::Method;
use std::collections::HashSet;

/// Type alias for HttpService with MethodEndpoint
pub type MethodRoute = HttpService<MethodEndpoint>;

pub struct MethodEndpoint {
    allowed_methods: HashSet<Method>,
}

impl MethodEndpoint {
    /// Create a new empty MethodEndpoint with no allowed methods
    pub fn new() -> Self {
        Self {
            allowed_methods: HashSet::new(),
        }
    }
    
    /// Add GET method to the allowed methods
    pub fn get(mut self) -> Self {
        self.allowed_methods.insert(Method::GET);
        self
    }
    
    /// Add POST method to the allowed methods
    pub fn post(mut self) -> Self {
        self.allowed_methods.insert(Method::POST);
        self
    }
    
    /// Add PUT method to the allowed methods
    pub fn put(mut self) -> Self {
        self.allowed_methods.insert(Method::PUT);
        self
    }
    
    /// Add DELETE method to the allowed methods
    pub fn delete(mut self) -> Self {
        self.allowed_methods.insert(Method::DELETE);
        self
    }
    
    /// Add PATCH method to the allowed methods
    pub fn patch(mut self) -> Self {
        self.allowed_methods.insert(Method::PATCH);
        self
    }
    
    /// Add OPTIONS method to the allowed methods
    pub fn options(mut self) -> Self {
        self.allowed_methods.insert(Method::OPTIONS);
        self
    }
    
    /// Add HEAD method to the allowed methods
    pub fn head(mut self) -> Self {
        self.allowed_methods.insert(Method::HEAD);
        self
    }
    
    /// Add TRACE method to the allowed methods
    pub fn trace(mut self) -> Self {
        self.allowed_methods.insert(Method::TRACE);
        self
    }
    
    /// Add CONNECT method to the allowed methods
    pub fn connect(mut self) -> Self {
        self.allowed_methods.insert(Method::CONNECT);
        self
    }
    
    /// Add a custom method to the allowed methods
    pub fn method(mut self, method: Method) -> Self {
        self.allowed_methods.insert(method);
        self
    }
    
    /// Add multiple methods from an iterator
    pub fn methods(mut self, methods: impl IntoIterator<Item = Method>) -> Self {
        self.allowed_methods.extend(methods);
        self
    }
}

/// Route module provides convenience functions for creating method-filtered routes
pub mod route {
    use super::{MethodEndpoint, MethodRoute};
    use crate::actor::HttpService;
    use crate::endpoint::Endpoint;
    
    /// Create an HttpService with GET method filter wrapping the given service
    pub fn get<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().get())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with POST method filter wrapping the given service
    pub fn post<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().post())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with PUT method filter wrapping the given service
    pub fn put<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().put())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with DELETE method filter wrapping the given service
    pub fn delete<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().delete())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with PATCH method filter wrapping the given service
    pub fn patch<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().patch())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with OPTIONS method filter wrapping the given service
    pub fn options<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().options())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with HEAD method filter wrapping the given service
    pub fn head<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().head())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with GET and POST method filters wrapping the given service
    pub fn get_or_post<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().get().post())
            .append_next(service.addr().clone().into())
    }
    
    /// Create an HttpService with RESTful methods (GET, POST, PUT, DELETE) wrapping the given service
    pub fn restful<E: Endpoint>(service: &HttpService<E>) -> MethodRoute 
    where
        E: 'static,
    {
        HttpService::new(MethodEndpoint::new().get().post().put().delete())
            .append_next(service.addr().clone().into())
    }
}

impl Default for MethodEndpoint {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Endpoint for MethodEndpoint {
    async fn route(
        &self,
        request: &Request,
        next: &[Address<HttpService>],
    ) -> Option<&Address<HttpService>> {
        // Check if the request method is in the allowed methods set
        if self.allowed_methods.contains(request.method()) {
            // Method matches, forward to the first next service
            next.first()
        } else {
            // Method doesn't match, handle locally (will return 405)
            None
        }
    }
    
    async fn handle_leaf(&self, request: &Request) -> Response {
        // Build Allow header with all allowed methods
        let allow_header = if self.allowed_methods.is_empty() {
            String::new()
        } else {
            self.allowed_methods
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        };
        
        // Return 405 Method Not Allowed for non-matching methods
        let mut builder = Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED);
        
        // Only add Allow header if there are allowed methods
        if !allow_header.is_empty() {
            builder = builder.header("Allow", &allow_header);
        }
        
        let body_message = if allow_header.is_empty() {
            format!("Method {} not allowed. No methods are allowed.", request.method())
        } else {
            format!(
                "Method {} not allowed. Allowed methods: {}",
                request.method(),
                allow_header
            )
        };
        
        builder
            .body(BoxBody::from(body_message))
            .unwrap()
    }
}
```

Key characteristics:
- **Builder Pattern**: Uses a fluent builder API for adding methods
- **Empty Initialization**: `new()` and `Default` create an empty endpoint with no allowed methods
- **Method Addition**: Methods like `get()`, `post()`, `options()` add methods to the allowed set and return `Self` for chaining
- **Type Alias**: `MethodRoute` is a type alias for `HttpService<MethodEndpoint>` for convenience
- **Multiple Methods Support**: Supports filtering for multiple HTTP methods simultaneously
- **Set-Based Matching**: Uses `HashSet` for efficient O(1) method lookup
- **Method Matching**: Checks if request method is in the allowed methods set
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Response**: Returns `405 Method Not Allowed` with `Allow` header listing all allowed methods (if any)
- **Empty Set Handling**: If no methods are added, all requests will return `405 Method Not Allowed` without an `Allow` header
- **Fluent API**: Methods return `Self` for method chaining, enabling readable code like `MethodEndpoint::new().get().post()`

### Route Module

The `route` module provides convenience functions for creating method-filtered routes that wrap existing services:

- **Single Method Filters**: `route::get()`, `route::post()`, `route::put()`, `route::delete()`, `route::patch()`, `route::options()`, `route::head()` - Create a method filter for a single HTTP method
- **Common Combinations**: `route::get_or_post()`, `route::restful()` - Create filters for common method combinations
- **Type Safety**: All functions accept `&HttpService<E>` and return `MethodRoute` (which is `HttpService<MethodEndpoint>`)
- **Service Reference**: The provided service's address (obtained via `addr()`) is added to the `next` vector of the created `MethodRoute`

These convenience functions simplify the common pattern of wrapping a service with a method filter:

```rust
// Instead of:
let handler = HttpService::new(MyEndpoint);
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);
let method_filter = MethodEndpoint::new().get();
let service = HttpService::new(method_filter)
    .append_next(handler_addr.into());

// You can write:
let handler = HttpService::new(MyEndpoint);
// Use the service reference directly - addr() is available after creation
let service = route::get(&handler);
let (addr, future) = service.start();
tokio::spawn(future);
// Start the handler service
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);
```

## Architecture

### Request Flow with MethodEndpoint

1. HTTP request arrives at `HttpService` with `MethodEndpoint`
2. `MethodEndpoint::route()` checks if request method is in the allowed methods set
3. If method matches (is in the allowed methods set):
   - Returns `Some(&Address)` pointing to the first service in `next`
   - Request is forwarded to that service
4. If method doesn't match:
   - Returns `None`
   - `handle_leaf()` is called, returning `405 Method Not Allowed` with `Allow` header listing all allowed methods

## Design Decisions

### Why Return None for Non-Matching Requests?

`MethodEndpoint` returns `None` from `route()` for non-matching requests:

- **Clear Semantics**: `None` clearly indicates the request should be handled locally
- **Error Handling**: Allows `handle_leaf()` to return appropriate error responses
- **Separation of Concerns**: Filtering logic is separate from error response generation
- **Consistency**: Follows the same pattern as other endpoints that handle requests locally

### Why Forward to First Service Only?

`MethodEndpoint` forwards to `next.first()`:

- **Simplicity**: Keeps the filtering logic simple and predictable
- **Single Target**: Filter endpoints typically have one downstream service
- **Composability**: Multiple filters can be chained to achieve complex routing
- **Flexibility**: More complex routing can be handled by dedicated router endpoints

### Why Use HashSet for Method Storage?

`MethodEndpoint` uses `HashSet` to store allowed methods:

- **Efficient Lookup**: O(1) average-case lookup time for method checking
- **No Duplicates**: Automatically handles duplicate methods when adding via builder methods
- **Flexible Construction**: Easy to build from iterators, arrays, or vectors using `methods()`
- **Standard Library**: Uses well-tested standard library types
- **Scalability**: Performance remains constant regardless of the number of allowed methods

### Why Use Builder Pattern Instead of Constructor Parameters?

`MethodEndpoint` uses a builder pattern with methods like `get()`, `post()`, etc.:

- **Fluent API**: Enables readable, chainable code: `MethodEndpoint::new().get().post()`
- **Flexibility**: Easy to add or remove methods without creating new constructors
- **Discoverability**: Method names clearly indicate which HTTP methods are being added
- **Extensibility**: Easy to add new HTTP methods without breaking existing code
- **Empty Initialization**: `new()` and `Default` allow starting with an empty set, useful for conditional method addition
- **Consistency**: All method-adding methods follow the same pattern, making the API predictable

## Examples

### Basic MethodEndpoint Usage

```rust
// Create a service that only accepts GET requests (single method)
let get_endpoint = MethodEndpoint::new().get();
let get_service = HttpService::new(get_endpoint)
    .append_next(handler_addr);

let (addr, future) = get_service.start();
tokio::spawn(future);

// GET request - will be forwarded
let get_request = Request::builder()
    .method(Method::GET)
    .uri("/api/data")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: get_request }).await?;
// Response comes from handler service

// POST request - will return 405
let post_request = Request::builder()
    .method(Method::POST)
    .uri("/api/data")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: post_request }).await?;
// Response is 405 Method Not Allowed with Allow: GET

// Create a service that accepts multiple methods using builder pattern
let multi_endpoint = MethodEndpoint::new()
    .get()
    .post()
    .put();
let multi_service = HttpService::new(multi_endpoint)
    .append_next(handler_addr);

let (multi_addr, multi_future) = multi_service.start();
tokio::spawn(multi_future);

// GET, POST, and PUT requests will all be forwarded
// DELETE request will return 405 with Allow: GET, POST, PUT

// Using Default trait
let default_endpoint = MethodEndpoint::default().get().post();
```

### Using Route Module Convenience Functions

```rust
use MethodEndpoint::route;

// Create handler service
let handler = HttpService::new(MyHandlerEndpoint);

// Create a GET-only service using convenience function
// The service's addr() method is used internally
let get_service = route::get(&handler);
let (addr, future) = get_service.start();
tokio::spawn(future);

// Start the handler service
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);

// Create a POST-only service
let post_service = route::post(&handler);
let (post_addr, post_future) = post_service.start();
tokio::spawn(post_future);

// Create a service that accepts both GET and POST
let get_or_post_service = route::get_or_post(&handler);
let (gop_addr, gop_future) = get_or_post_service.start();
tokio::spawn(gop_future);

// Create a RESTful service (GET, POST, PUT, DELETE)
let restful_service = route::restful(&handler);
let (restful_addr, restful_future) = restful_service.start();
tokio::spawn(restful_future);

// Convenience functions work with any service reference
let path_service = HttpService::new(PathEndpoint::new("api"));
let api_get_service = route::get(&path_service);
let (api_get_addr, api_get_future) = api_get_service.start();
tokio::spawn(api_get_future);
let (path_addr, path_future) = path_service.start();
tokio::spawn(path_future);
```

### Multiple Methods Support

```rust
// Create endpoint that allows multiple methods using builder pattern
let endpoint = MethodEndpoint::new().get().post();
let service = HttpService::new(endpoint)
    .append_next(handler_addr);

let (addr, future) = service.start();
tokio::spawn(future);

// GET request - allowed
let get_request = Request::builder()
    .method(Method::GET)
    .uri("/api/data")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: get_request }).await?;

// POST request - allowed
let post_request = Request::builder()
    .method(Method::POST)
    .uri("/api/data")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: post_request }).await?;

// PUT request - returns 405
let put_request = Request::builder()
    .method(Method::PUT)
    .uri("/api/data")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: put_request }).await?;
// Response is 405 Method Not Allowed with Allow: GET, POST

// Create endpoint with multiple methods using builder pattern
let custom_endpoint = MethodEndpoint::new()
    .get()
    .post()
    .put()
    .delete();
let custom_service = HttpService::new(custom_endpoint)
    .append_next(handler_addr);

let (custom_addr, custom_future) = custom_service.start();
tokio::spawn(custom_future);

// All four methods will be allowed

// Using methods() to add from iterator
let iter_endpoint = MethodEndpoint::new()
    .methods([Method::GET, Method::POST, Method::PUT]);
    
// Using method() to add custom methods
let custom_method = Method::from_bytes(b"CUSTOM").unwrap();
let custom_endpoint = MethodEndpoint::new().method(custom_method);
```

### Composing with Other Endpoints

```rust
// Create handler service
let handler = HttpService::new(BusinessLogicEndpoint);
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);

// Create path filter chain: api -> users
let users_endpoint = HttpService::new(PathEndpoint::new("users"))
    .append_next(handler_addr.into());
let (users_addr, users_future) = users_endpoint.start();
tokio::spawn(users_future);

let api_endpoint = HttpService::new(PathEndpoint::new("api"))
    .append_next(users_addr.into());
let (api_addr, api_future) = api_endpoint.start();
tokio::spawn(api_future);

// Create method filter (wraps path filter) - allows GET and POST
let method_filter = HttpService::new(MethodEndpoint::new().get().post())
    .append_next(api_addr.into());
let (root_addr, root_future) = method_filter.start();
tokio::spawn(root_future);

// Only GET and POST requests to /api/users will reach the handler
// Request flow: /api/users -> method check -> /api/users -> "api" segment -> /users -> "users" segment -> /
let get_request = Request::builder()
    .method(Method::GET)
    .uri("/api/users")
    .body(BoxBody::empty())
    .unwrap();
let response = root_addr.call(HttpRequest { request: get_request }).await?;

let post_request = Request::builder()
    .method(Method::POST)
    .uri("/api/users")
    .body(BoxBody::empty())
    .unwrap();
let response = root_addr.call(HttpRequest { request: post_request }).await?;
```

## Integration with Actor Model

### MethodEndpoint as Actor

`MethodEndpoint` participates in the actor system through `HttpService`:

- **Service Trait**: `HttpService` with `MethodEndpoint` implements `Service` trait
- **Message Handling**: Handles `HttpRequest` messages through the `Handler` trait
- **Async Processing**: All filtering and forwarding operations are asynchronous
- **Address**: Can be addressed via `Address<HttpService>`

### Request Flow Through MethodEndpoint

1. HTTP request arrives as `HttpRequest` message
2. `HttpService` calls `endpoint.route()` with the request
3. `MethodEndpoint` checks if request method is in the allowed methods set
4. If match: request is forwarded to next service via `Address::call()`
5. If no match: `handle_leaf()` generates error response
6. Response flows back through the service chain

## Future Considerations

- **Custom Error Responses**: Allow customization of error response bodies
- **Method Patterns**: Support for method patterns or ranges
- **Filter Composition**: Helper types for composing multiple filters more easily
- **Performance**: Consider optimizations for common method sets (e.g., RESTful methods)
