# Method and Path Endpoints

## Overview

This document describes the implementation of `MethodEndpoint` and `PathEndpoint`, two specialized endpoint implementations that provide HTTP method and path filtering capabilities. These endpoints act as middleware-like filters that check request properties and either forward matching requests to downstream services or return error responses for non-matching requests.

## Core Concepts

### Filter Endpoints

`MethodEndpoint` and `PathEndpoint` are specialized `Endpoint` implementations that:

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

The `PathEndpoint` filters requests based on URI path segments:

- **Purpose**: Ensures only requests matching a specific path segment are allowed to pass through
- **Segment-Based Matching**: Matches only the first segment of the request path, not the entire path
- **Path Decomposition**: To match a path like `/a/b/c`, multiple `PathEndpoint` instances are needed (one for `a`, one for `b`, one for `c`)
- **Catch-All Support**: Provides a catch-all mode that matches any remaining path segments
- **Forwarding**: Forwards matching requests to the first service in `next` with the matched segment removed from the path
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
- **Multiple Methods Support**: Supports filtering for multiple HTTP methods simultaneously
- **Set-Based Matching**: Uses `HashSet` for efficient O(1) method lookup
- **Method Matching**: Checks if request method is in the allowed methods set
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Response**: Returns `405 Method Not Allowed` with `Allow` header listing all allowed methods (if any)
- **Empty Set Handling**: If no methods are added, all requests will return `405 Method Not Allowed` without an `Allow` header
- **Fluent API**: Methods return `Self` for method chaining, enabling readable code like `MethodEndpoint::new().get().post()`

### 2. PathEndpoint Implementation

The `PathEndpoint` filters requests by matching a single path segment:

```rust
pub struct PathEndpoint {
    segment: Option<String>,
    catch_all: bool,
}

impl PathEndpoint {
    /// Create a new PathEndpoint that matches a specific path segment
    pub fn new(segment: impl Into<String>) -> Self {
        Self {
            segment: Some(segment.into()),
            catch_all: false,
        }
    }
    
    /// Create a catch-all PathEndpoint that matches any remaining path segments
    pub fn catch_all() -> Self {
        Self {
            segment: None,
            catch_all: true,
        }
    }
    
    /// Extract the first segment from a path
    fn extract_first_segment(path: &str) -> Option<&str> {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return None;
        }
        path.split('/').next()
    }
    
    /// Remove the first segment from a path
    fn remove_first_segment(path: &str) -> String {
        let path = path.trim_start_matches('/');
        if path.is_empty() {
            return String::from("/");
        }
        let segments: Vec<&str> = path.split('/').collect();
        if segments.len() <= 1 {
            String::from("/")
        } else {
            format!("/{}", segments[1..].join("/"))
        }
    }
    
    /// Check if the first segment of the request path matches
    fn matches(&self, request_path: &str) -> bool {
        if self.catch_all {
            // Catch-all matches any non-empty path
            !request_path.trim_start_matches('/').is_empty()
        } else if let Some(ref segment) = self.segment {
            // Match the first segment
            Self::extract_first_segment(request_path)
                .map(|first| first == segment.as_str())
                .unwrap_or(false)
        } else {
            false
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
        
        // Check if the first segment matches
        if self.matches(request_path) {
            // Path segment matches, forward to the first next service
            // Note: The actual path modification should be handled by HttpService
            // when forwarding the request, removing the matched segment
            next.first()
        } else {
            // Path doesn't match, handle locally (will return 404)
            None
        }
    }
    
    async fn handle_leaf(&self, request: &Request) -> Response {
        let request_path = request.uri().path();
        let expected = if self.catch_all {
            "any remaining path segments"
        } else if let Some(ref segment) = self.segment {
            format!("path segment: {}", segment)
        } else {
            "a path segment".to_string()
        };
        
        // Return 404 Not Found for non-matching paths
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(BoxBody::from(format!(
                "Path not found: {}. Expected {}",
                request_path,
                expected
            )))
            .unwrap()
    }
}
```

Key characteristics:
- **Segment-Based Matching**: Matches only the first segment of the request path
- **Single Segment**: Each `PathEndpoint` matches exactly one path segment
- **Path Decomposition**: Multiple `PathEndpoint` instances are chained to match multi-segment paths
- **Catch-All Mode**: `catch_all()` creates an endpoint that matches any remaining path segments
- **Path Modification**: When forwarding, the matched segment should be removed from the request path
- **Forwarding**: Forwards matching requests to the first service in `next`
- **Error Response**: Returns `404 Not Found` for non-matching paths

**Note on Path Modification**: The `route()` method returns `Some(&Address)` when a segment matches, but the actual path modification (removing the matched segment) should be handled by `HttpService` when forwarding the request. This can be implemented by:
- Modifying the request URI before forwarding
- Using request extensions to track path segments
- Creating a new request with the modified path

The `remove_first_segment()` helper method is provided to assist with path modification, but the actual implementation depends on how `HttpService` handles request forwarding.

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
2. `PathEndpoint::route()` extracts the first segment from the request path
3. `PathEndpoint::route()` checks if the first segment matches the configured segment (or catch-all)
4. If segment matches:
   - Returns `Some(&Address)` pointing to the first service in `next`
   - The matched segment is removed from the request path
   - Request is forwarded to that service with the modified path
5. If segment doesn't match:
   - Returns `None`
   - `handle_leaf()` is called, returning `404 Not Found`

### Path Segment Processing

When a `PathEndpoint` matches and forwards a request:

- **Segment Extraction**: The first segment is extracted from the request path (e.g., `/a/b/c` → `a`)
- **Segment Matching**: The extracted segment is compared with the endpoint's configured segment
- **Path Modification**: If matched, the segment is removed from the path (e.g., `/a/b/c` → `/b/c`)
- **Forwarding**: The modified request is forwarded to the next service
- **Catch-All**: If using `catch_all()`, all remaining segments are consumed and the path becomes `/`

### Composition Pattern

`MethodEndpoint` and `PathEndpoint` are designed to be composed:

```
HttpService (MethodEndpoint::new().get().post())
└── next: [Address<HttpService>]
    └── HttpService (PathEndpoint::new("api"))
        └── next: [Address<HttpService>]
            └── HttpService (PathEndpoint::new("users"))
                └── next: [Address<HttpService>]
                    └── HttpService (BusinessLogicEndpoint)
```

In this example:
1. `MethodEndpoint` filters for GET and POST requests (using builder pattern)
2. First `PathEndpoint` matches the `api` segment (`/api/users` → `/users`)
3. Second `PathEndpoint` matches the `users` segment (`/users` → `/`)
4. Only GET and POST requests to `/api/users` reach the business logic endpoint

### Multi-Segment Path Matching

To match a path like `/a/b/c`, you need three `PathEndpoint` instances:

```
HttpService (PathEndpoint::new("a"))
└── next: [Address<HttpService>]
    └── HttpService (PathEndpoint::new("b"))
        └── next: [Address<HttpService>]
            └── HttpService (PathEndpoint::new("c"))
                └── next: [Address<HttpService>]
                    └── HttpService (HandlerEndpoint)
```

Request flow:
- Request `/a/b/c` arrives at first `PathEndpoint`
- First segment `a` matches, path becomes `/b/c`, forwarded to second `PathEndpoint`
- Second segment `b` matches, path becomes `/c`, forwarded to third `PathEndpoint`
- Third segment `c` matches, path becomes `/`, forwarded to handler

## Design Decisions

### Why Return None for Non-Matching Requests?

`MethodEndpoint` and `PathEndpoint` return `None` from `route()` for non-matching requests:

- **Clear Semantics**: `None` clearly indicates the request should be handled locally
- **Error Handling**: Allows `handle_leaf()` to return appropriate error responses
- **Separation of Concerns**: Filtering logic is separate from error response generation
- **Consistency**: Follows the same pattern as other endpoints that handle requests locally

### Why Forward to First Service Only?

Both `MethodEndpoint` and `PathEndpoint` forward to `next.first()`:

- **Simplicity**: Keeps the filtering logic simple and predictable
- **Single Target**: These endpoints typically have one downstream service
- **Composability**: Multiple filters can be chained to achieve complex routing
- **Flexibility**: More complex routing can be handled by dedicated router endpoints

### Why Segment-Based Matching Instead of Full Path Matching?

`PathEndpoint` matches only a single path segment instead of the full path:

- **Composability**: Enables building complex routes by composing multiple `PathEndpoint` instances
- **Modularity**: Each endpoint handles one segment, making the routing logic clear and testable
- **Flexibility**: Easy to create different route combinations by chaining different segments
- **Path Decomposition**: Naturally decomposes paths into segments, matching how URLs are structured
- **Reusability**: Individual segment endpoints can be reused in different route configurations
- **Explicit Routing**: Makes it explicit which segment is being matched at each level

### Why Support Catch-All?

`PathEndpoint` provides a `catch_all()` mode:

- **Flexible Routing**: Allows matching any remaining path segments after specific segments
- **Wildcard Behavior**: Useful for routes that need to accept variable path segments
- **Simplified Handling**: Avoids needing to create endpoints for every possible path segment
- **Common Pattern**: Matches common web framework patterns (e.g., `/api/*` or `/static/*`)

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

### Basic PathEndpoint Usage

```rust
// Create a service that matches the "api" segment
let api_endpoint = PathEndpoint::new("api");
let api_service = HttpService::new(api_endpoint)
    .append_next(api_handler_addr);

let (addr, future) = api_service.start();
tokio::spawn(future);

// Request to /api - first segment "api" matches, path becomes "/"
let api_request = Request::builder()
    .uri("/api")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: api_request }).await?;
// Response comes from api_handler service

// Request to /static - first segment "static" doesn't match "api", returns 404
let static_request = Request::builder()
    .uri("/static")
    .body(BoxBody::empty())
    .unwrap();
let response = addr.call(HttpRequest { request: static_request }).await?;
// Response is 404 Not Found

// Using catch-all to match any remaining segments
let catch_all_endpoint = PathEndpoint::catch_all();
let catch_all_service = HttpService::new(catch_all_endpoint)
    .append_next(handler_addr);

let (catch_all_addr, catch_all_future) = catch_all_service.start();
tokio::spawn(catch_all_future);

// Request to /anything/here/will/match - catch-all matches all segments
let any_request = Request::builder()
    .uri("/anything/here/will/match")
    .body(BoxBody::empty())
    .unwrap();
let response = catch_all_addr.call(HttpRequest { request: any_request }).await?;
// Response comes from handler service
```

### Composing Filters

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

### Multi-Segment Path Matching

```rust
// Create endpoint chain for /api/v1/users path
let users_handler = HttpService::new(UsersHandlerEndpoint);
let (users_handler_addr, users_handler_future) = users_handler.start();
tokio::spawn(users_handler_future);

// Third segment: "users"
let users_endpoint = HttpService::new(PathEndpoint::new("users"))
    .append_next(users_handler_addr.into());
let (users_addr, users_future) = users_endpoint.start();
tokio::spawn(users_future);

// Second segment: "v1"
let v1_endpoint = HttpService::new(PathEndpoint::new("v1"))
    .append_next(users_addr.into());
let (v1_addr, v1_future) = v1_endpoint.start();
tokio::spawn(v1_future);

// First segment: "api"
let api_endpoint = HttpService::new(PathEndpoint::new("api"))
    .append_next(v1_addr.into());
let (api_addr, api_future) = api_endpoint.start();
tokio::spawn(api_future);

// Request to /api/v1/users - matches all segments
let users_request = Request::builder()
    .uri("/api/v1/users")
    .body(BoxBody::empty())
    .unwrap();
let response = api_addr.call(HttpRequest { request: users_request }).await?;
// Request flow: /api/v1/users -> "api" -> /v1/users -> "v1" -> /users -> "users" -> /
// Forwarded to handler

// Request to /api/v1/posts - "users" segment doesn't match, returns 404
let posts_request = Request::builder()
    .uri("/api/v1/posts")
    .body(BoxBody::empty())
    .unwrap();
let response = api_addr.call(HttpRequest { request: posts_request }).await?;
// Returns 404 Not Found at the "users" endpoint level
```

### Catch-All Usage

```rust
// Create endpoint chain: api -> catch-all
let handler = HttpService::new(ApiHandlerEndpoint);
let (handler_addr, handler_future) = handler.start();
tokio::spawn(handler_future);

let catch_all_endpoint = HttpService::new(PathEndpoint::catch_all())
    .append_next(handler_addr.into());
let (catch_all_addr, catch_all_future) = catch_all_endpoint.start();
tokio::spawn(catch_all_future);

let api_endpoint = HttpService::new(PathEndpoint::new("api"))
    .append_next(catch_all_addr.into());
let (api_addr, api_future) = api_endpoint.start();
tokio::spawn(api_future);

// Request to /api/anything/here - "api" matches, catch-all matches the rest
let any_request = Request::builder()
    .uri("/api/anything/here")
    .body(BoxBody::empty())
    .unwrap();
let response = api_addr.call(HttpRequest { request: any_request }).await?;
// Request flow: /api/anything/here -> "api" -> /anything/here -> catch-all -> /
// Forwarded to handler
```

## Integration with Actor Model

### MethodEndpoint and PathEndpoint as Actors

`MethodEndpoint` and `PathEndpoint` participate in the actor system through `HttpService`:

- **Service Trait**: `HttpService` with these endpoints implements `Service` trait
- **Message Handling**: Handles `HttpRequest` messages through the `Handler` trait
- **Async Processing**: All filtering and forwarding operations are asynchronous
- **Address**: Can be addressed via `Address<HttpService>`

### Request Flow Through MethodEndpoint and PathEndpoint

1. HTTP request arrives as `HttpRequest` message
2. `HttpService` calls `endpoint.route()` with the request
3. For `MethodEndpoint`: checks if request method is in the allowed methods set
4. For `PathEndpoint`: extracts first segment from path and checks if it matches the configured segment (or catch-all)
5. If match:
   - For `PathEndpoint`: the matched segment is removed from the request path
   - Request is forwarded to next service via `Address::call()` with modified request
6. If no match: `handle_leaf()` generates error response
7. Response flows back through the service chain

## Future Considerations

### PathEndpoint Enhancements

- **Regex Segment Matching**: Support for regex-based segment patterns (e.g., match segments starting with "user")
- **Path Parameters**: Extract path parameters from matching segments (e.g., `:id` pattern)
- **Case Sensitivity**: Configurable case sensitivity for segment matching
- **Trailing Slash Handling**: Configurable handling of trailing slashes in paths
- **Path Normalization**: Normalize paths before segment extraction (e.g., remove duplicate slashes)
- **Performance**: Consider using `&str` instead of `String` for segment storage to avoid allocations
- **Segment Validation**: Support for validating segment format (e.g., numeric IDs, UUIDs)

### General Enhancements

- **Custom Error Responses**: Allow customization of error response bodies for both `MethodEndpoint` and `PathEndpoint`
- **Filter Composition**: Helper types for composing multiple filters more easily (e.g., `PathChain` builder)
- **Method Patterns**: Support for method patterns or ranges in `MethodEndpoint`
- **Path Modification API**: Standardized way to modify request paths when forwarding (e.g., via Request extensions)
- **Segment Extraction Utilities**: Helper functions for common path manipulation operations
