# Endpoint Filters

## Overview

This document describes the implementation of filter endpoints that provide HTTP method and path filtering capabilities. These endpoints act as middleware-like filters that check request properties and either forward matching requests to downstream services or return error responses for non-matching requests.

## Core Concepts

### Filter Endpoints

Filter endpoints are specialized `Endpoint` implementations that:

- **Filter Requests**: Check specific request properties (HTTP method or path)
- **Conditional Forwarding**: Forward requests to next services only if they match the filter criteria
- **Error Responses**: Return appropriate error responses (e.g., `405 Method Not Allowed` or `404 Not Found`) for non-matching requests
- **Composability**: Can be composed with other endpoints to build complex routing logic

### MethodEndpoint

The `MethodEndpoint` filters requests based on HTTP method:

- **Purpose**: Ensures only specific HTTP methods are allowed to pass through
- **Multiple Methods Support**: Supports filtering for one or multiple HTTP methods simultaneously
- **Filtering Logic**: Checks if the request method is in the set of allowed methods
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Handling**: Returns `405 Method Not Allowed` with `Allow` header listing all allowed methods for non-matching requests

### PathEndpoint

The `PathEndpoint` filters requests based on URI path:

- **Purpose**: Ensures only requests matching a specific path pattern are allowed to pass through
- **Filtering Logic**: Checks if the request path matches the configured path pattern
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Handling**: Returns `404 Not Found` for non-matching paths

## Design Principles

### 1. MethodEndpoint Implementation

The `MethodEndpoint` filters requests by HTTP method, supporting multiple allowed methods:

```rust
use http::Method;
use std::collections::HashSet;

pub struct MethodEndpoint {
    allowed_methods: HashSet<Method>,
}

impl MethodEndpoint {
    /// Create a new MethodEndpoint that allows multiple HTTP methods
    pub fn new(methods: impl IntoIterator<Item = Method>) -> Self {
        Self {
            allowed_methods: methods.into_iter().collect(),
        }
    }
    
    /// Create a MethodEndpoint for a single method (convenience)
    pub fn single(method: Method) -> Self {
        Self::new([method])
    }
    
    /// Create a MethodEndpoint for GET requests
    pub fn get() -> Self {
        Self::single(Method::GET)
    }
    
    /// Create a MethodEndpoint for POST requests
    pub fn post() -> Self {
        Self::single(Method::POST)
    }
    
    /// Create a MethodEndpoint for PUT requests
    pub fn put() -> Self {
        Self::single(Method::PUT)
    }
    
    /// Create a MethodEndpoint for DELETE requests
    pub fn delete() -> Self {
        Self::single(Method::DELETE)
    }
    
    /// Create a MethodEndpoint for PATCH requests
    pub fn patch() -> Self {
        Self::single(Method::PATCH)
    }
    
    /// Create a MethodEndpoint that allows both GET and POST
    pub fn get_or_post() -> Self {
        Self::new([Method::GET, Method::POST])
    }
    
    /// Create a MethodEndpoint that allows GET, POST, PUT, and DELETE
    pub fn restful() -> Self {
        Self::new([Method::GET, Method::POST, Method::PUT, Method::DELETE])
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
        let allow_header = self.allowed_methods
            .iter()
            .map(|m| m.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        
        // Return 405 Method Not Allowed for non-matching methods
        Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .header("Allow", &allow_header)
            .body(BoxBody::from(format!(
                "Method {} not allowed. Allowed methods: {}",
                request.method(),
                allow_header
            )))
            .unwrap()
    }
}
```

Key characteristics:
- **Multiple Methods Support**: Supports filtering for multiple HTTP methods simultaneously
- **Set-Based Matching**: Uses `HashSet` for efficient O(1) method lookup
- **Method Matching**: Checks if request method is in the allowed methods set
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Response**: Returns `405 Method Not Allowed` with `Allow` header listing all allowed methods
- **Convenience Constructors**: Provides helper methods for common HTTP methods (GET, POST, etc.) and common combinations

### 2. PathEndpoint Implementation

The `PathEndpoint` filters requests by URI path:

```rust
pub struct PathEndpoint {
    path_pattern: String,
    exact_match: bool,
}

impl PathEndpoint {
    /// Create a new PathEndpoint with exact path matching
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path_pattern: path.into(),
            exact_match: true,
        }
    }
    
    /// Create a PathEndpoint with prefix matching
    pub fn prefix(path: impl Into<String>) -> Self {
        Self {
            path_pattern: path.into(),
            exact_match: false,
        }
    }
    
    /// Check if the request path matches this endpoint's pattern
    fn matches(&self, request_path: &str) -> bool {
        if self.exact_match {
            request_path == self.path_pattern
        } else {
            request_path.starts_with(&self.path_pattern)
        }
    }
}

#[async_trait]
impl Endpoint for PathEndpoint {
    async fn route(
        &self,
        request: &Request,
        next: &[Address<HttpService>],
    ) -> Option<&Address<HttpService>> {
        let request_path = request.uri().path();
        
        // Check if the request path matches the pattern
        if self.matches(request_path) {
            // Path matches, forward to the first next service
            next.first()
        } else {
            // Path doesn't match, handle locally (will return 404)
            None
        }
    }
    
    async fn handle_leaf(&self, request: &Request) -> Response {
        // Return 404 Not Found for non-matching paths
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::from(format!(
                "Path not found: {}. Expected pattern: {}",
                request.uri().path(),
                self.path_pattern
            )))
            .unwrap()
    }
}
```

Key characteristics:
- **Path Pattern Matching**: Supports both exact matching and prefix matching
- **Exact Match Mode**: Matches the entire path exactly (default)
- **Prefix Match Mode**: Matches paths that start with the pattern
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Response**: Returns `404 Not Found` for non-matching paths
- **Flexible Construction**: Provides both exact and prefix matching constructors

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

### Request Flow with PathEndpoint

1. HTTP request arrives at `HttpService` with `PathEndpoint`
2. `PathEndpoint::route()` checks if request path matches the pattern
3. If path matches:
   - Returns `Some(&Address)` pointing to the first service in `next`
   - Request is forwarded to that service
4. If path doesn't match:
   - Returns `None`
   - `handle_leaf()` is called, returning `404 Not Found`

### Composition Pattern

Filter endpoints are designed to be composed:

```
HttpService (MethodEndpoint::get_or_post())
└── next: [Address<HttpService>]
    └── HttpService (PathEndpoint::prefix("/api"))
        └── next: [Address<HttpService>]
            └── HttpService (BusinessLogicEndpoint)
```

In this example:
1. `MethodEndpoint` filters for GET and POST requests
2. `PathEndpoint` filters for paths starting with `/api`
3. Only GET and POST requests to `/api/*` reach the business logic endpoint

## Design Decisions

### Why Return None for Non-Matching Requests?

Filter endpoints return `None` from `route()` for non-matching requests:

- **Clear Semantics**: `None` clearly indicates the request should be handled locally
- **Error Handling**: Allows `handle_leaf()` to return appropriate error responses
- **Separation of Concerns**: Filtering logic is separate from error response generation
- **Consistency**: Follows the same pattern as other endpoints that handle requests locally

### Why Forward to First Service Only?

Both filter endpoints forward to `next.first()`:

- **Simplicity**: Keeps the filtering logic simple and predictable
- **Single Target**: Filter endpoints typically have one downstream service
- **Composability**: Multiple filters can be chained to achieve complex routing
- **Flexibility**: More complex routing can be handled by dedicated router endpoints

### Why Separate Exact and Prefix Matching?

`PathEndpoint` supports both exact and prefix matching:

- **Exact Matching**: Useful for specific routes (e.g., `/api/health`)
- **Prefix Matching**: Useful for route groups (e.g., `/api/users/*`)
- **Flexibility**: Covers common routing patterns without over-engineering
- **Clear API**: Separate constructors (`new()` vs `prefix()`) make intent clear

### Why Use HashSet for Method Storage?

`MethodEndpoint` uses `HashSet` to store allowed methods:

- **Efficient Lookup**: O(1) average-case lookup time for method checking
- **No Duplicates**: Automatically handles duplicate methods when constructing from iterators
- **Flexible Construction**: Easy to build from iterators, arrays, or vectors
- **Standard Library**: Uses well-tested standard library types
- **Scalability**: Performance remains constant regardless of the number of allowed methods

## Examples

### Basic MethodEndpoint Usage

```rust
// Create a service that only accepts GET requests (single method)
let get_endpoint = MethodEndpoint::get();
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

// Create a service that accepts multiple methods
let multi_endpoint = MethodEndpoint::new([Method::GET, Method::POST, Method::PUT]);
let multi_service = HttpService::new(multi_endpoint)
    .append_next(handler_addr);

let (multi_addr, multi_future) = multi_service.start();
tokio::spawn(multi_future);

// GET, POST, and PUT requests will all be forwarded
// DELETE request will return 405 with Allow: GET, POST, PUT
```

### Basic PathEndpoint Usage

```rust
// Create a service that only accepts requests to /api/*
let api_endpoint = PathEndpoint::prefix("/api");
let api_service = HttpService::new(api_endpoint)
    .append_next(api_handler_addr);

let (addr, future) = api_service.start();
tokio::spawn(future);

// Request to /api/users - will be forwarded
let api_request = Request::builder()
    .uri("/api/users")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: api_request }).await?;
// Response comes from api_handler service

// Request to /static/file - will return 404
let static_request = Request::builder()
    .uri("/static/file")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: static_request }).await?;
// Response is 404 Not Found
```

### Composing Filters

```rust
// Create handler service
let handler = HttpService::new(BusinessLogicEndpoint);
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);

// Create path filter
let path_filter = HttpService::new(PathEndpoint::prefix("/api"))
    .append_next(handler_addr.into());
let (path_addr, path_future) = path_filter.start();
tokio::spawn(path_future);

// Create method filter (wraps path filter) - allows GET and POST
let method_filter = HttpService::new(MethodEndpoint::get_or_post())
    .append_next(path_addr.into());
let (root_addr, root_future) = method_filter.start();
tokio::spawn(root_future);

// Only GET and POST requests to /api/* will reach the handler
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

### Multiple Methods Support

```rust
// Create endpoint that allows multiple methods using convenience constructor
let endpoint = MethodEndpoint::get_or_post();
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

// Create endpoint with custom method set
let custom_endpoint = MethodEndpoint::new([
    Method::GET,
    Method::POST,
    Method::PUT,
    Method::DELETE,
]);
let custom_service = HttpService::new(custom_endpoint)
    .append_next(handler_addr);

let (custom_addr, custom_future) = custom_service.start();
tokio::spawn(custom_future);

// All four methods will be allowed
```

### Exact Path Matching

```rust
// Create endpoint with exact path matching
let health_endpoint = PathEndpoint::new("/health");
let health_service = HttpService::new(health_endpoint)
    .append_next(health_handler_addr);

let (addr, future) = health_service.start();
tokio::spawn(future);

// Request to /health - matches exactly
let health_request = Request::builder()
    .uri("/health")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: health_request }).await?;
// Forwarded to handler

// Request to /health/check - doesn't match (exact match)
let check_request = Request::builder()
    .uri("/health/check")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: check_request }).await?;
// Returns 404 Not Found
```

## Integration with Actor Model

### Filter Endpoints as Actors

Filter endpoints participate in the actor system through `HttpService`:

- **Service Trait**: `HttpService` with filter endpoints implements `Service` trait
- **Message Handling**: Handles `HttpRequest` messages through the `Handler` trait
- **Async Processing**: All filtering and forwarding operations are asynchronous
- **Address**: Can be addressed via `Address<HttpService>`

### Request Flow Through Filters

1. HTTP request arrives as `HttpRequest` message
2. `HttpService` calls `endpoint.route()` with the request
3. Filter endpoint checks request properties (method or path)
4. If match: request is forwarded to next service via `Address::call()`
5. If no match: `handle_leaf()` generates error response
6. Response flows back through the service chain

## Future Considerations

- **Regex Path Matching**: Support for regex-based path patterns in `PathEndpoint`
- **Wildcard Patterns**: Support for wildcard patterns (e.g., `/api/*/users`)
- **Path Parameters**: Extract path parameters from matching paths
- **Case Sensitivity**: Configurable case sensitivity for path matching
- **Trailing Slash Handling**: Configurable handling of trailing slashes
- **Custom Error Responses**: Allow customization of error response bodies
- **Filter Composition**: Helper types for composing multiple filters more easily
- **Performance**: Consider using `&str` instead of `String` for path patterns to avoid allocations
- **Path Normalization**: Normalize paths before matching (e.g., remove duplicate slashes)
